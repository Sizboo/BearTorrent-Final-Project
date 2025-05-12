use std::collections::HashMap;
use std::net::{ SocketAddr};
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use std::time::Duration;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{Connection, Endpoint, TokioRuntime};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use crate::torrent_client::TorrentClient;
use crate::connection::connection::{PeerId, CertMessage, Cert, InfoHash};
use crate::message::Message;
use tokio::{net::UdpSocket as TokioUdpSocket};
use tokio::sync::mpsc::Sender;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tonic::Request;
use crate::file_handler::{read_piece_from_file};

pub struct QuicP2PConn {
    endpoint: Endpoint,
}

impl QuicP2PConn {

    ///create_quic_server
    ///
    /// parameters:
    ///    - TokioUdpSocket: resembles the socket to consume in the connection
    ///    - peer_id: resembles the peer to accept connections from
    ///    - server: the connection to introducer server to send certificate
    ///    - cert_ip: the ip to build the certificate off (self ip)
    ///
    /// function:
    /// This creates a quinn server endpoint to accept a quic connection.
    pub(crate) async fn create_quic_server(
        socket: TokioUdpSocket,
        peer_id: PeerId,
        server: TorrentClient,
        cert_ip: String,
    ) -> Result<QuicP2PConn, Box<dyn std::error::Error>> {

        let (certs, key) = {
            println!("generating self-signed certificate");
            // let cert_ip = Ipv4Addr::from(torrent_client.self_addr.ipaddr).to_string();
            let cert = rcgen::generate_simple_self_signed(vec![cert_ip])?;
            let key = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());
            let cert: CertificateDer = cert.cert.into();

            (vec![cert], key.secret_pkcs8_der().to_vec())
        };

        //send certificate to client here
        let cert_bytes = certs[0].to_vec();
        let mut server_connection = server.client.clone();
        let request = Request::new( CertMessage{
            peer_id: Some(peer_id),
            cert: Some(Cert { certificate: cert_bytes }),
        });
        println!("Server sent client certificate!");


        server_connection.send_cert(request).await?;


        let mut server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, PrivateKeyDer::try_from(key.clone())?)?;
        //set my custom expected ALPN (Application-Layer Protocol Negotiation)
        server_crypto.alpn_protocols = vec![b"helpful-serf-p2p".to_vec()];

        //todo consider adding keylogging

        let server_config = quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(server_crypto)?));

        //todo consider setting max-uni-streams if that's necessary for us (control security)
        // let socket = socket_arc.take().into();

        let endpoint = Endpoint::new(
            quinn::EndpointConfig::default(),
            Some(server_config),
            socket.into_std().unwrap(),
            Arc::new(TokioRuntime),
        )?;

        Ok(
            QuicP2PConn {
                endpoint,
            }
        )
    }

    ///create_quic_client
    ///
    /// parameters:
    ///    - TokioUdpSocket: resembles the socket to consume in the connection
    ///    - self_addr: holds data of own connection ips
    ///    - server: the connection to introducer server to get certificate from
    ///
    /// function:
    /// This creates a quinn client endpoint to create a quic connection.
    pub (crate) async fn create_quic_client (
        socket: TokioUdpSocket,
        self_addr: PeerId,
        server: TorrentClient,
    ) -> Result<QuicP2PConn, Box<dyn std::error::Error>> {
        let mut server_connection = server.client.clone();

        let request = Request::new(self_addr);
        let cert_res = server_connection.get_cert(request).await?.into_inner();
        let cert_bytes = cert_res.certificate;

        println!("Client received certificate {:?}", cert_bytes);

        let mut roots = rustls::RootCertStore::empty();

        roots.add(CertificateDer::try_from(&cert_bytes[..])?)?;

        let mut client_crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        //set crypto with custom protocol type
        client_crypto.alpn_protocols = vec![b"helpful-serf-p2p".to_vec()];

        let client_config = quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(client_crypto)?));

        let mut endpoint = Endpoint::new(
            quinn::EndpointConfig::default(),
            None,
            socket.into_std().unwrap(),
            Arc::new(TokioRuntime),
        )?;

        endpoint.set_default_client_config(client_config);

        Ok( QuicP2PConn {
            endpoint,
        })
    }
    
    ///quic_listener
    ///
    /// parameters:
    ///    - file_map: this is the map used to get file information when it is requested by peer
    ///
    /// function:
    /// This method listens for a connection request, spawning off the send_data task when a connection
    /// is successfully made. it times out after 4 seconds if no connection request is made.
    pub(crate) async fn quic_listener(
        &mut self,
        file_map: Arc<RwLock<HashMap<[u8; 20], InfoHash>>>
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn_listener = self.endpoint.accept().await.ok_or("failed to accept")?;
        
        //establish timeout duration to exit if connection request is not received
        let timeout_duration = Duration::from_secs(4);

        println!("Listening on {:?}", self.endpoint.local_addr());
        let res = timeout(timeout_duration,conn_listener).await?;
        
        match res {
            Ok(conn) => {
                tokio::spawn(async move {
                    let res = QuicP2PConn::send_data(conn, file_map).await;
                    if res.is_err() {
                        eprintln!("Failed to get connection request Listener: {:?}", res);
                    }
                });
                
                Ok(())
            },
            Err(e) => {
                Err(Box::from(e.to_string()))
            }
        }
    }
    
    ///send_data()
    ///
    /// parameters:
    ///    - file_map: this is the file map from which file information is acquired when file
    ///                is requested.
    ///
    /// function:
    /// This method waits for incoming streams. It then takes the requests from the peer and then send
    /// the requested piece. If the piece is not available, it will respond with a Cancel request indicating
    /// the peer should ask another peer for the data.
    async fn send_data(
        conn: Connection,
        file_map: Arc<RwLock<HashMap<[u8; 20], InfoHash>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Seeder accepted quic connection");
        loop {
            tokio::select! {
                _ = conn.closed() => {
                    println!("Connection closed");
                    return Ok(());
                },
                stream = conn.accept_bi() => {
                    match stream {
                        Ok((mut send, mut recv)) => {
                           println!("Seeder accepted bi stream!");

                            let mut req_buf : [u8; 41] = [0; 41];
                            recv.read_exact(&mut req_buf).await?;
                            println!("Client received req {:?}", req_buf);

                            let mut request = None;
                            if let Some(msg) = Message::decode(Vec::from(req_buf)) {
                                request = match msg {
                                    Message::Request { seeder, index, begin, length, hash } => Some((seeder, index, begin, length, hash)),
                                    _ => None,
                                };
                            }
                            let (seeder, index, begin, length, hash) = request.ok_or("failed to decode request")?;

                            //if no message found, we send a Cancel message back indicating we do not have the piece
                            //the client will then re-issue this request to another peer.
                            let msg = match file_map.read().await.get(&hash).cloned(){
                                Some(info_hash) => {
                                    match read_piece_from_file(info_hash, index) {
                                        Ok(piece) => Message::Piece { index, piece },
                                        Err(_) => Message::Cancel {seeder, index, begin, length},
                                    }
                                },
                                None => Message::Cancel {seeder, index, begin, length},
                            };

                            send.write_all(&msg.encode()).await?;
                            send.finish()?;
                        },
                        Err(e) => return match e {
                            quinn::ConnectionError::ApplicationClosed(closed) => {
                                println!("Connection Closed: {:?}", closed.reason);
                                Ok(())
                            },
                            other => {
                                Err(Box::from(other.to_string()))
                            }
                        }
                    }
                }
            }
        }

    }

    ///connect_to_peer_server
    ///
    /// parameter:
    ///     - peer_addr: the is the address of the peer to connect to
    ///     - conn_rx: this is the receiving end of the file assembler channel from which to get requests from
    ///     - conn_tx: this is the sending end of the file assembler channel from which to send responses
    ///
    /// function:
    /// This method tries to connect to the peer quic server. If it succeeds, it spins off the recv_data task
    /// which sends file piece requests. It will timeout and return a failure in 4 seconds if it does not
    /// succeed in making a connection.
    pub(crate) async fn connect_to_peer_server(
        &mut self,
        peer_addr: SocketAddr,
        conn_tx: Sender<Message>,
        conn_rx: Arc<Mutex<Receiver<Message>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {

        let timeout_duration = Duration::from_secs(4);
        
        let res = timeout(timeout_duration,self.endpoint.connect(peer_addr, &*peer_addr.ip().to_string())?).await?;
        
        match res {
            Ok(conn) => {
                tokio::spawn(async move {
                    let res = QuicP2PConn::recv_data(conn, conn_tx, conn_rx).await;
                    if res.is_err() {
                        eprintln!("Connect to Peer Server Error{:?}", res);
                    }
                });
            
                Ok(()) 
            }
            Err(e) => {
                Err(Box::new(e))
            }
            
        }
        
    }

    ///recv_data
    ///
    /// parameters:
    ///    - conn_tx: the sending end of channel to send peer responses to reassembly_loop
    ///    - conn_rx: the receiving end of the channel to receive requests from request sender.
    ///
    /// function:
    /// This method loops through all the requests delegated to it by the receiver. It sends those
    /// to the peer and passes the response back up to the requester. If the connection fails,
    /// it loops back all the responses as a cancel request so they may be re-requested by another peer.
    async fn recv_data(
        conn: Connection,
        conn_tx: Sender<Message>,
        conn_rx: Arc<Mutex<Receiver<Message>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            if let Some(msg) = conn_rx.lock().await.recv().await {

                match conn.open_bi().await {
                    Ok((mut send, mut recv)) => {
                        println!("requester opened bi stream!");
                        let conn_tx_clone = conn_tx.clone();


                        let (index, length) = match msg {
                            Message::Request { index,length,.. } => (index, length),
                            _ => Err("length not found")?,
                        };

                        let ret: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> =
                            tokio::spawn(async move {
                                println!("requester waiting for length {:?}", length + 9);
                                let buf = recv.read_to_end(length as usize + 9).await?;
                                println!("received piece from peer");

                                let msg = Message::decode(buf).ok_or("failed to decode message")?;

                                conn_tx_clone.send(msg).await?;

                                Ok(())
                            });

                        send.write_all(&msg.encode()).await?;
                        send.finish()?;
                        println!("sent message requesting: {}", index);

                        let res = ret.await?;
                        if res.is_err() {
                            eprintln!("{:?}", res);
                        }
                    },
                    //if connection errors, we want to resend the requests to another connection
                    //this loops it back to file_assembler so it can handle removing this connection
                    //and resend a new request to a viable connection
                    Err(_) => {
                        println!("Connection Error sending Cancel Request");
                        let cancel_req = match msg {
                            Message::Request { seeder, index, begin, length, .. } =>
                                Message::Cancel { seeder, index, begin, length },
                            _ => continue,
                        };
                        conn_tx.send(cancel_req).await?;
                    }
                }

            } else {
                println!("connection closed");
                conn.close(0u32.into(), b"closing connection gracefully");
                return Ok(())
            }
        }
    }
}