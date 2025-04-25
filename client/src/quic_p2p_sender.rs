use std::fs;
use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::sync::Arc;
use quinn::crypto::rustls::QuicServerConfig;
use quinn::{Endpoint, TokioRuntime};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use crate::torrent_client::TorrentClient;
use tokio::net::UdpSocket as TokioUdpSocket;


pub struct P2PSender {
    endpoint: Endpoint
}

impl P2PSender {
    fn new(endpoint: Endpoint) -> P2PSender {
        P2PSender { endpoint }
    }
    pub(crate) async fn create_quic_server(torrent_client: &mut TorrentClient, socket: TokioUdpSocket) -> Result<P2PSender, Box<dyn std::error::Error>> {
        //create and establish self-sign certificates - based of quinn-rs example

        let (certs, key) = {
            //establish platform-specific paths
            let dirs = directories_next::ProjectDirs::from("org", "helpful_serf", "torrent_client")
                .ok_or_else(|| Box::<dyn std::error::Error>::from("Could not get project dirs"))?;
            let path = dirs.data_local_dir();
            let cert_path = path.join("cert.der");
            let key_path = path.join("key.der");

            //get certificates or create new ones
            let (cert, key) = match fs::read(&cert_path).and_then(|x| Ok((x, fs::read(&key_path)?))) {
                Ok((cert, key)) => (
                    CertificateDer::from(cert),
                    PrivateKeyDer::try_from(key).map_err(|e| Box::<dyn std::error::Error>::from(e))?,
                ),
                Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                    println!("generating self-signed certificate");
                    let cert_ip = Ipv4Addr::from(torrent_client.self_addr.ipaddr).to_string();
                    let cert = rcgen::generate_simple_self_signed(vec![cert_ip])?;
                    let key = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());
                    let cert = cert.cert.into();

                    fs::create_dir_all(path).map_err(|e| Box::<dyn std::error::Error>::from(e))?;
                    fs::write(&cert_path, &cert).map_err(|e| Box::<dyn std::error::Error>::from(e))?;
                    fs::write(&key_path, key.secret_pkcs8_der()).map_err(|e| Box::<dyn std::error::Error>::from(e))?;

                    (cert, key.into())
                }
                _ => {
                    return Err(Box::new(std::io::Error::new(ErrorKind::Unsupported, "failed to create or retrieve cert")))
                }
            };

            (vec![cert], key)
        };

        let mut server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
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
            P2PSender::new(endpoint)
        )
    }
    pub(crate) async fn send_data(&self) {
        let conn = self.endpoint.accept().await.unwrap();

        println!("Connection Received!");

    }
}