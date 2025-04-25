use std::{error, fs};
use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::sync::Arc;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{Endpoint, TokioRuntime};
use quinn::Side::Server;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use crate::server_connection::ServerConnection;
use crate::torrent_client::{connection, TorrentClient};
use tokio::net::UdpSocket as TokioUdpSocket;
use tonic::Request;
use connection::{CertMessage, PeerId, Cert};

pub struct QuicP2PConn {
    endpoint: Endpoint,
    private_key: Option<Vec<u8>>,
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
        server: ServerConnection
    ) -> Result<QuicP2PConn, Box<dyn std::error::Error>> {
        
        let (certs, key) = {
            println!("generating self-signed certificate");
            let cert_ip = Ipv4Addr::from(torrent_client.self_addr.ipaddr).to_string();
            let cert = rcgen::generate_simple_self_signed(vec![cert_ip])?;
            let key = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());
            let cert: CertificateDer = cert.cert.into();

            (vec![cert], key.secret_pkcs8_der().to_vec()) 
        };

        //send certificate to client here
        let cert_bytes = certs[0].to_vec();
        let mut server_connection = server.client.clone();
        let request = Request::new( connection::CertMessage{
            peer_id: Some(peer_id),
            cert: Some(Cert { certificate: cert_bytes }),
        });
        println!("Server sent client certificate!");
        
        
        let res = server_connection.send_cert(request).await?;
        println!("Self Signed send response {:?}", res);
        

        let mut server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, PrivateKeyDer::try_from(key.clone())?)?;
        //set my custom expected ALPN (Application-Layer Protocol Negotiation)
        server_crypto.alpn_protocols = vec![b"helpful-serf-p2p".to_vec()];

        //todo consider adding keylogging

        let server_config = quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(server_crypto)?));

        //todo consider setting max-uni-streams if thats necessary for us (control security)
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
        
        roots.add(CertificateDer::try_from(&cert_bytes[..])?).map_err(|e|  Err(Box::new(e)));
        
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
        })
    }
    pub(crate) async fn send_data(&self) {
        let conn = self.endpoint.accept().await.unwrap();

        println!("Connection Received!");

    }
}