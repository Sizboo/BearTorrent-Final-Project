use std::net::{IpAddr, ToSocketAddrs};
// use local_ip_address::local_ip;
use stunclient::StunClient;
use tokio::net::UdpSocket;
use tonic::transport::{Channel, ClientTlsConfig};
use crate::torrent_client::connection::{PeerId, connector_client::ConnectorClient, ClientId};
use crate::torrent_client::TorrentClient;

#[derive(Debug, Clone)]
pub struct ServerConnection {
    pub(crate) client: ConnectorClient<Channel>,
    pub(crate) uid: Option<ClientId>,
}

const GCLOUD_URL: &str = "https://helpful-serf-server-1016068426296.us-south1.run.app:";

impl ServerConnection {
    pub (crate) async fn new() -> Result<ServerConnection, Box<dyn std::error::Error>> {
        //tls config
        //webki roots uses Mozilla's certificate store
        let tls = ClientTlsConfig::new()
            .with_webpki_roots()
            .domain_name("helpful-serf-server-1016068426296.us-south1.run.app");

        let endpoint = Channel::from_static(GCLOUD_URL).tls_config(tls)?
            .connect().await?;

        let client = ConnectorClient::new(endpoint);

        //todo use this to figure out id persistence across sessions
        //1. get uuid
        // let dirs = directories_next::ProjectDirs::from("org", "helpful_serf", "torrent_client")
        //     .ok_or_else(|| Box::<dyn std::error::Error>::from("Could not get project dirs"))?;
        // let path = dirs.data_local_dir();
        // let uid_path = path.join("uuid.der");
        // 
        // let mut uid= "".to_string();
        // 
        // if uid_path.exists() {
        //     uid = std::fs::read_to_string(&uid_path)?;
        //     
        //     //todo verify server has uid
        // }
        // //2. get new uid from server 
        // else {
        //     
        // }
        //update id information
        // fs::create_dir_all(&path)?;
        // fs::write(&uid_path, uid.clone())?;

        Ok(
            ServerConnection {
                client,
                uid: None,
            }
        )
    }

    pub async fn register_server_connection(&mut self, self_addr: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        let uid = self.client.register_client(self_addr).await?;
        self.uid = Some(uid.into_inner());

        Ok(())
    }

    pub (crate) async fn create_client(&mut self) -> Result<TorrentClient, Box<dyn std::error::Error>> {

        //bind port and get public facing id
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        let stun_server = "stun.l.google.com:19302".to_socket_addrs().unwrap().filter(|x|x.is_ipv4()).next().unwrap();
        
        //TEST
        // socket.connect(&stun_server)?;
        
        let client = StunClient::new(stun_server);
        let external_addr = client.query_external_address(&socket)?;

        let pub_ipaddr =  match external_addr.ip() {
            IpAddr::V4(v4) => Ok(u32::from_be_bytes(v4.octets())),
            IpAddr::V6(_) => Err("Cannot convert IPv6 to u32"),
        }?;

        println!("My public IP {}", external_addr.ip());
        println!("My public PORT {}", external_addr.port());

        let local_addr = socket.local_addr()?;
        
        let priv_ipaddr = match local_addr.ip(){
            IpAddr::V4(ip) => Ok(u32::from_be_bytes(ip.octets())),
            IpAddr::V6(ip) => Err("Cannot convert IPv6 to u32"),
        }?;
        
        let priv_port = local_addr.port() as u32;

        println!("My private IP {}", local_addr.ip());
        println!("My private PORT {}", priv_port);
        
        let self_addr = PeerId {
            pub_ipaddr,
            pub_port: external_addr.port() as u32,
            priv_ipaddr,
            priv_port

        };
        socket.set_nonblocking(true)?;

        self.register_server_connection(self_addr.clone()).await?;

        let server = self.clone();

        Ok(
            TorrentClient::new(server, Some(UdpSocket::from_std(socket)?), self_addr)
        )
    }
}
