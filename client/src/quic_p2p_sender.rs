use std::fs;
use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::sync::Arc;
use quinn::crypto::rustls::QuicServerConfig;
use quinn::{Endpoint, TokioRuntime};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use crate::server_connection::ServerConnection;
use crate::torrent_client::{connection, TorrentClient};
use tokio::net::UdpSocket as TokioUdpSocket;
use tonic::Request;
use connection::{CertMessage, PeerId, Cert};

pub struct P2PSender {
    endpoint: Endpoint,
    private_key: Vec<u8>,
}

impl P2PSender {

    pub(crate) async fn init_for_certificate(
        self, 
        self_addr: PeerId,
        server: ServerConnection,
    ) {
        let mut server_connection = server.client.clone();
        
        let request = Request::new(self_addr);
        
        let res = server_connection.init_cert_sender(request).await;
        eprintln!("Response from init for certificate {:?}", res)
    }
    pub(crate) async fn create_quic_server(
        torrent_client: &mut TorrentClient, 
        socket: TokioUdpSocket, 
        peer_id: PeerId,
        server: ServerConnection
    ) -> Result<P2PSender, Box<dyn std::error::Error>> {
        
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
            P2PSender {
                endpoint,
                private_key: key,
            }
        )
    }
    pub(crate) async fn send_data(&self) {
        let conn = self.endpoint.accept().await.unwrap();

        println!("Connection Received!");

    }
}