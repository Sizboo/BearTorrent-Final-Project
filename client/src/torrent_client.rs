use std::fs;
use std::io::{Error, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use stunclient::StunClient;
use std::net::UdpSocket;
use tokio::net::UdpSocket as TokioUdpSocket;
use std::sync::Arc;
use std::time::Duration;
use quinn::crypto::rustls::QuicServerConfig;
use quinn::{Endpoint, TokioRuntime};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::time::sleep;
use tokio::{join, try_join};
use tonic::{Request, Response};
use tonic::service::interceptor::ResponseBody;
use tonic::transport::{Channel, ClientTlsConfig, Server};
use connection::{PeerId, connector_client::ConnectorClient};
use crate::connection::FileMessage;

pub mod connection {
    tonic::include_proto!("connection");
}

const GCLOUD_URL: &str = "https://helpful-serf-server-1016068426296.us-south1.run.app:";

#[derive(Debug, Clone)]
pub struct ServerConnection {
    client: ConnectorClient<Channel>,
    identifier: u32,
}

impl ServerConnection {
    pub async fn new() -> Result<ServerConnection, Box<dyn std::error::Error>> {
        //tls config
        //webki roots uses Mozilla's certificate store
        let tls = ClientTlsConfig::new()
            .with_webpki_roots()
            .domain_name("helpful-serf-server-1016068426296.us-south1.run.app");

        let endpoint = Channel::from_static(GCLOUD_URL).tls_config(tls)?
            .connect().await?;
        //todo add identifier and its associated implementation on server

        Ok( 
            ServerConnection {
                client: ConnectorClient::new(endpoint),
            }
        )
    }
}

pub struct P2PSender {
    endpoint: Endpoint
}

impl P2PSender {
    fn new(endpoint: Endpoint) -> P2PSender {
        P2PSender { endpoint }
    }
    async fn create_quic_server(torrent_client: &mut TorrentClient, socket: TokioUdpSocket) -> Result<P2PSender, Box<dyn std::error::Error>> {
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
    async fn send_data(&self) {
        let conn = self.endpoint.accept().await.unwrap();
        
        println!("Connection Received!");
        
    }
}

#[derive(Debug)]
pub struct TorrentClient {
    server: ServerConnection,
    socket: Option<TokioUdpSocket>,
    self_addr: PeerId,
}

impl TorrentClient {

    pub async fn new(server: ServerConnection) -> Result<TorrentClient, Box<dyn std::error::Error>> {

        //bind port and get public facing id
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        let stun_server = "stun.l.google.com:19302".to_socket_addrs().unwrap().filter(|x|x.is_ipv4()).next().unwrap();
        let client = StunClient::new(stun_server);
        let external_addr = client.query_external_address(&socket)?;

        let ipaddr =  match external_addr.ip() {
            IpAddr::V4(v4) => Ok(u32::from_be_bytes(v4.octets())),
            IpAddr::V6(_) => Err("Cannot convert IPv6 to u32"),
        }?;

        println!("My public IP {}", external_addr.ip());
        println!("My public PORT {}", external_addr.port());

        let self_addr = PeerId {
            ipaddr,
            port: external_addr.port() as u32,
        };
        socket.set_nonblocking(true)?;
        

        socket.set_nonblocking(true)?;
        
        Ok(
            TorrentClient {
                server,
                socket: Some(TokioUdpSocket::from_std(socket)?) ,
                self_addr,
            },
        )
    }


    async fn hole_punch(&mut self, peer_id: PeerId ) -> Result<TokioUdpSocket, Box<dyn std::error::Error>> {
        let ip_addr = Ipv4Addr::from(peer_id.ipaddr);
        let port = peer_id.port as u16;
        let peer_addr = SocketAddr::from((ip_addr, port));
        
        //todo maybe don't take this here
        let socket_arc = Arc::new(self.socket.take().unwrap());
        let socket_clone = socket_arc.clone();

        let punch_string = b"HELPFUL_SERF";

        println!("Starting Send to peer ip: {}, port: {}", ip_addr, port);
        
        let send_task = tokio::spawn(async move {
            for i in 0..50 {
                let res = socket_arc.send_to(punch_string, peer_addr).await;
                
                if res.is_err() {
                    println!("Send Failed: {}", res.err().unwrap());
                }
                
                sleep(Duration::from_millis(10)).await;
            }
        });


        let read_task = tokio::spawn( async move {
            let mut recv_buf = [0u8; 1024];
            loop {
                match socket_clone.recv_from(&mut recv_buf).await {
                    Ok((n, src)) => {
                        println!("Received from {}: {:?}", src, &recv_buf[..n]);
                        if &recv_buf[..n] == punch_string {
                            println!("Punched SUCCESS {}", src);
                            
                            //todo cancel send process and break

                            return Arc::try_unwrap(socket_clone).map_err(|_| Box::<dyn std::error::Error + Send + Sync>::from("socket arc is still in use"));
                        }
                    }
                    Err(e) => {
                        eprintln!("Recv error: {}", e);
                        return Err(Box::<dyn std::error::Error + Send + Sync>::from(e));
                    },
                }
            }
        });

        try_join!(read_task, send_task);
        
        Err(Box::new(std::io::Error::new(ErrorKind::TimedOut, "hole punch timed out")))
    }

    ///seeding is used as a listening process to begin sending data upon request
    /// it simply awaits a server request for it to send data
    pub async fn seeding(server: ServerConnection) -> Result<(), Box<dyn std::error::Error>> {
        
        loop {
            let mut torrent_client = TorrentClient::new(server.clone()).await?;

            let mut server_client = torrent_client.server.client.clone();
            
            // calls get_peer
            let response = server_client.get_peer(torrent_client.self_addr).await;

            // waits for response from get_peer
            match response {
                Ok(res) => {

                    tokio::spawn(async move {
                        let res = torrent_client.connect_to_peer(res).await; 
                        if res.is_err() {
                            println!("Connect Failed: {}", res.err().unwrap());
                        }
                    });
               
                }
                Err(e) => {
                    eprintln!("Failed to get peer: {:?}", e);
                }
            }
        }
    }
    
    pub async fn connect_to_peer(&mut self, res: Response<PeerId>) -> Result<(), Box<dyn std::error::Error>> {
        let peer_id = res.into_inner();
        
        println!("peer to send {:?}", peer_id);

        //todo 1. try connection over local NAT


        //todo 2. try hole punch
        //hole punch
        let socket = self.hole_punch(peer_id).await?;
        //start quick server
        let p2p_sender = P2PSender::create_quic_server(self, socket).await?;
        p2p_sender.send_data().await;
        //todo 3 TURN 
        
        Ok(())
    }

    ///request is a method used to request necessary connection details from the server
    pub async fn file_request(&self, file_hash: u32) -> Result<connection::PeerList, Box<dyn std::error::Error>> {
        let mut client = self.server.client.clone();
        
        
        let request = Request::new(connection::FileMessage {
            id: Some(self.self_addr),
            info_hash: file_hash,
        });
        
        let resp = client.send_file_request(request).await?;
        
        Ok(resp.into_inner())
    }

    ///Used when client is requesting a file
    pub async fn get_file_from_peer(&mut self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        self.hole_punch(peer_id).await?;

        Ok(())
    }

    ///download is a process used to retrieve data from a peer
    /// parameters:
    /// peer ip and port
    ///
    /// download will initiate UDP hole punching from receiving end
    ///  then receive data
    pub async fn download(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }


    /// announce is a method used update server with connection details
    /// this will primarily be used by init() and called on a periodic basis
    /// this is essentially a "keep alive" method
    pub async fn announce(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    pub async fn advertise(&self) -> Result<PeerId, Box<dyn std::error::Error>> {
        let mut client = self.server.client.clone();
        
        //todo make hash active
        let request = Request::new(connection::FileMessage {
            id: Some(self.self_addr),
            info_hash: 12345,
        });
        
        let resp = client.advertise(request).await?;
        
        Ok(resp.into_inner())
    }
    

}
