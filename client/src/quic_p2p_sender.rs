use std::any::Any;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use std::time::Duration;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{Connection, Endpoint, TokioRuntime};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use crate::server_connection::ServerConnection;
use crate::torrent_client::TorrentClient;
use crate::connection::connection::{PeerId, CertMessage, Cert};
use crate::message::Message;
use tokio::{net::UdpSocket as TokioUdpSocket, sync::mpsc};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;
use tonic::Request;
use crate::file_handler::{read_piece_from_file, InfoHash};

pub struct QuicP2PConn {
    endpoint: Endpoint,
}

impl QuicP2PConn {

    /// init_for_certificate enables a quic client to receive the server's self-signed certificate
    /// this must be called before a client can retrieve a certificate and make a quic connection
    /// the sooner this is called the better.
    /// By convention, the receiving peer is the client and will ideally call this prior to hole punching
    pub(crate) async fn init_for_certificate(
        self,
        self_addr: PeerId,
        server: ServerConnection,
    ) {
        let mut server_connection = server.client.clone();

        let request = Request::new(self_addr);

        let res = server_connection.init_cert_sender(request).await;
        println!("Client init certificate {:?}", res);
        eprintln!("Response from init for certificate {:?}", res)
    }

    pub(crate) async fn create_quic_server(
        socket: TokioUdpSocket,
        peer_id: PeerId,
        server: ServerConnection,
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


        let res = server_connection.send_cert(request).await?;

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

    pub (crate) async fn create_quic_client (
        socket: TokioUdpSocket,
        self_addr: PeerId,
        server: ServerConnection,
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
               
                //todo do this until figure out static bind
                QuicP2PConn::send_data(conn, file_map).await?;
                
                Ok(())
            },
            Err(e) => {
                return Err(Box::new(e));
            }
        }
    }
    
    async fn send_data(
        conn: Connection,
        file_map: Arc<RwLock<HashMap<[u8; 20], InfoHash>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Seeder accepted quic connection");
        let (mut send, mut recv) = conn.accept_bi().await?;
        println!("Seeder accepted bi stream!");
        
        loop {
            let mut req_buf : [u8; 33] = [0; 33];
            recv.read_exact(&mut req_buf).await?;
            println!("Client received req {:?}", req_buf);

            let mut request = None;
            if let Some(msg) = Message::decode(Vec::from(req_buf)) {
                request = match msg {
                    Message::Request { index, begin, length, hash } => Some((index, begin, length, hash)),
                    _ => None,
                };
            }
            let (index, begin, length, hash) = request.ok_or("failed to decode request")?;

            let info_hash = file_map.read().await.get(&hash).cloned().ok_or("seeder missing file info")?;


            let piece = read_piece_from_file(info_hash, index)?;

            let msg = Message::Piece { index, piece };


            send.write_all(&msg.encode()).await?;
            send.finish()?;
            println!("Seeder sent piece {:?}", msg);
        }
        
        conn.closed().await;
        println!("sending end quic connection closed");
        
        Ok(())
        
    }

    pub(crate) async fn connect_to_peer_server(
        &mut self,
        peer_addr: SocketAddr,
        conn_tx: Sender<Message>,
        conn_rx: Arc<Mutex<Receiver<Message>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {

        // TODO pass off data to data handler... seems like it isn't looping thru data yet.. do that first
        let timeout_duration = Duration::from_secs(4);
        
        let res = timeout(timeout_duration,self.endpoint.connect(peer_addr, &*peer_addr.ip().to_string())?).await?;
        
        match res {
            Ok(conn) => {
                tokio::spawn(async move {
                    let res = QuicP2PConn::recv_data(conn, conn_tx, conn_rx).await;
                    if res.is_err() {
                        eprintln!("{:?}", res);
                    }
                });
                Ok(()) 
            }
            Err(e) => {
                return Err(Box::new(e));
            }
            
        }
        
    }
    
    async fn recv_data(
        conn: Connection,
        conn_tx: Sender<Message>,
        conn_rx: Arc<Mutex<Receiver<Message>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        
        let (mut send, mut recv) = conn.open_bi().await?;
        println!("requester opened bi stream!");
        loop {
            if let Some(msg) = conn_rx.lock().await.recv().await {
                
                println!("Message received of length: {:?}", msg);
                send.write_all(&msg.encode()).await?;
                println!("sent message");

                let length: Result<u32, Box<dyn std::error::Error>> = match msg {
                    Message::Request { length, .. } => Ok(length),
                    _ => Err("length not found".into()),
                };
                let length = length?;

                let piece = recv.read_to_end(length as usize + 9).await?;
                println!("received piece from per");

                let msg = Message::decode(piece).ok_or("failed to decode message")?;
 
                conn_tx.send(msg).await?;
            }
        }
        
        conn.closed().await;
        println!("receiving end quic connection closed");
        
        Ok(())
        
    }
}