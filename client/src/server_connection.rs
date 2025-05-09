use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddrV4, ToSocketAddrs};
use local_ip_address::local_ip;
use stunclient::StunClient;
use tokio::net::UdpSocket;
use tonic::transport::{Channel, ClientTlsConfig};
use crate::connection::connection::{connector_client, turn_client, ClientId, ClientRegistry, FullId, PeerId};
use crate::file_handler::{get_info_hashes, InfoHash};
use crate::torrent_client::TorrentClient;

#[derive(Debug, Clone)]
pub struct ServerConnection {
    pub(crate) client: connector_client::ConnectorClient<Channel>,
    pub(crate) turn: turn_client::TurnClient<Channel>,
    pub(crate) uid: ClientId,
    pub(crate) file_hashes: HashMap<[u8;20], InfoHash>
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

        let mut client = connector_client::ConnectorClient::new(endpoint.clone());
        let turn = turn_client::TurnClient::new(endpoint);
        
        let uid = client.register_client(ClientRegistry { peer_id: None} ).await?;
        let uid = uid.into_inner();

        let file_hashes = match get_info_hashes(){
            Ok(file_hashes) => file_hashes,
            Err(err) => return Err(Box::new(err)),
        };

        Ok(
            ServerConnection {
                client,
                turn,
                uid,
                file_hashes,
            }
        )
    }
    pub async fn register_new_client(&mut self) -> Result<TorrentClient, Box<dyn std::error::Error>> {
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

        // Get the local IP address of this machine (defaults to Ipv4)
        let priv_ipaddr = match local_ip() {
            Ok(ip) => {
                match ip {
                    IpAddr::V4(v4) => v4,
                    IpAddr::V6(_) => return Err(Box::new(std::io::Error::new(ErrorKind::Other, "Cannot convert IPv6 to u32"))),
                }
            }
            Err(err) => return Err(Box::new(err)),
        };

        let priv_port = 50000;

        println!("My private IP {:?}", priv_ipaddr);
        println!("My private port is {}", priv_port);

        let priv_socket = UdpSocket::bind(SocketAddrV4::new(priv_ipaddr, priv_port)).await?;

        let self_addr = PeerId {
            ipaddr,
            port: external_addr.port() as u32,
            priv_ipaddr: u32::from_be_bytes(priv_ipaddr.octets()),
            priv_port: priv_port as u32,
        };
        socket.set_nonblocking(true)?;

        self.update_registered_peer_id(self_addr.clone()).await?;

        let server = self.clone();

        Ok(
            TorrentClient {
                server,
                pub_socket: Some(UdpSocket::try_from(socket)?) ,
                priv_socket: Some(priv_socket),
                self_addr,
            },
        ) 
    }
    async fn update_registered_peer_id(&mut self, self_addr: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        
        let mut server_conn = self.client.clone();
        
        
        match server_conn.update_registered_peer_id(
            FullId { 
                self_id: Option::from(self.uid.clone()), 
                peer_id: Some(self_addr)
            }
        ).await {
            Ok(res) => Ok(()),
            Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
        }
        
    }
}
