use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{Connection, Endpoint, TokioRuntime};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use crate::server_connection::ServerConnection;
use crate::torrent_client::TorrentClient;
use crate::connection::connection::{PeerId, CertMessage, Cert};
use crate::message::Message;
use tokio::{net::UdpSocket as TokioUdpSocket, sync::mpsc};
use tokio::time::timeout;
use tonic::Request;

pub struct QuicP2PConn {
    endpoint: Endpoint,
    private_key: Option<Vec<u8>>,
    /// the sender we use to send to a PeerConnection
    conn_tx: mpsc::Sender<Message>,
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
        torrent_client: &mut TorrentClient,
        socket: TokioUdpSocket,
        peer_id: PeerId,
        server: ServerConnection,
        cert_ip: String,
        conn_tx: mpsc::Sender<Message>
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
                private_key: Some(key),
                conn_tx
            }
        )
    }

    pub (crate) async fn create_quic_client (
        socket: TokioUdpSocket,
        self_addr: PeerId,
        server: ServerConnection,
        conn_tx: mpsc::Sender<Message>
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
            private_key: None,
            conn_tx
        })
    }
    
    //todo quic_listener will have to take channel_rx and pass to send_data to send data from connection to our implementation
    pub(crate) async fn quic_listener(&self)
    -> Result<(), Box<dyn std::error::Error>> {
        let conn_listener = self.endpoint.accept().await.ok_or("failed to accept")?;
        
        //establish timeout duration to exit if connection request is not received
        let timeout_duration = Duration::from_secs(4);

        println!("Listening on {:?}", self.endpoint.local_addr());
        let res = timeout(timeout_duration,conn_listener).await?;
        
        match res {
            Ok(conn) => {
                let send_task = tokio::spawn(async move {
                    let res = QuicP2PConn::send_data(conn).await;
                    if res.is_err() {
                        eprintln!("QuicP2PConn::send_data failed {:?}", res);
                    }
                });

                println!("Connection received on {:?}", self.endpoint.local_addr());
                send_task.await?;
                Ok(())
            },
            Err(e) => {
                return Err(Box::new(e));
            }
        }
    }
    
    async fn send_data(conn: Connection) -> Result<(), Box<dyn std::error::Error>> {
        let mut sender = conn.open_uni().await?;

        sender.write_all(b"sending data directly to my peer!").await?;
        sender.finish()?;
        
        conn.closed().await;
        println!("sending end quic connection closed");
        
        Ok(())
        
    }

    pub(crate) async fn connect_to_peer_server(&self, peer_addr: SocketAddr)
    -> Result<(), Box<dyn std::error::Error>> {

        // TODO pass off data to data handler... seems like it isn't looping thru data yet.. do that first
        let timeout_duration = Duration::from_secs(4);
        
        let res = timeout(timeout_duration,self.endpoint.connect(peer_addr, &*peer_addr.ip().to_string())?).await?;
        
        match res {
            Ok(conn) => {
                let conn_for_read = conn.clone();
                let tx_for_read   = self.conn_tx.clone();
                let read_task = tokio::spawn(async move {
                    let res = QuicP2PConn::recv_data(conn_for_read, tx_for_read).await;
                    if res.is_err() {
                        eprintln!("QuicP2PConn::recv_data failed {:?}", res);
                    }
                });

                read_task.await?;
                Ok(()) 
            }
            Err(e) => {
                return Err(Box::new(e));
            }
            
        }
        
    }
    
    async fn recv_data(conn: Connection, conn_tx: mpsc::Sender<Message>) -> Result<(), Box<dyn std::error::Error>> {

        let mut recv = conn.accept_uni().await?;
        let mut buf = vec![0u8; 1024];
        let n = recv.read(&mut buf).await?.unwrap();

        conn_tx.send(buf);
        
        conn.closed().await;
        println!("receiving end quic connection closed");
        
        Ok(())
        
    }
}