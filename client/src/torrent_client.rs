use std::cmp::min;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddrV4, ToSocketAddrs};
use std::sync::Arc;
use local_ip_address::local_ip;
use stunclient::StunClient;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tonic::Request;
use tonic::transport::{Channel, ClientTlsConfig};
use crate::connection::connection::{connector_client, turn_client, ClientId, ClientRegistry, FileMessage, FullId, PeerId, InfoHash};
use crate::file_assembler::FileAssembler;
use crate::file_handler::{get_info_hashes};
use crate::peer_connection::PeerConnection;

#[derive(Debug, Clone)]
pub struct TorrentClient {
    pub(crate) client: connector_client::ConnectorClient<Channel>,
    pub(crate) turn: turn_client::TurnClient<Channel>,
    pub(crate) uid: ClientId,
    pub(crate) file_hashes: Arc<RwLock<HashMap<[u8;20], InfoHash>>>
}

const GCLOUD_URL: &str = "https://helpful-serf-server-1016068426296.us-south1.run.app:";

impl TorrentClient {
    pub (crate) async fn new() -> Result<TorrentClient, Box<dyn std::error::Error>> {
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
            TorrentClient {
                client,
                turn,
                uid,
                file_hashes: Arc::new(RwLock::new(file_hashes)),
            }
        )
    }
    async fn register_new_connection(&mut self) -> Result<PeerConnection, Box<dyn std::error::Error>> {
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


        let priv_addr= SocketAddrV4::new(priv_ipaddr, 0);
        let priv_socket = UdpSocket::bind(priv_addr).await?;

        let priv_port = priv_socket.local_addr()?.port();

        println!("My private IP {:?}", priv_ipaddr);
        println!("My private port is {}", priv_port);


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
            PeerConnection {
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

    ///seeding is used as a listening process to begin sending data upon request
    /// it simply awaits a server request for it to send data
    pub async fn seeding(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        loop {
            let mut peer_connection = self.register_new_connection().await?;

            let mut server_client = self.client.clone();

            //update server's list of seeder files
            self.advertise_all().await?;

            // calls get_peer
            let response = server_client.seed(peer_connection.self_addr.clone()).await;

            // waits for response from get_peer
            match response {
                Ok(res) => {
                    tokio::spawn(async move {
                        let res = peer_connection.seeder_connection(res).await;
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

    ///request is a method used to request necessary connection details from the server
    pub async fn file_request(&mut self, file_hash: InfoHash) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = self.client.clone();

        let peer_list = client.get_file_peer_list(file_hash.clone()).await?.into_inner().list;

        //we want to maximize connection which means either one connection per piece
        // or one connection per peer whichever is less.
        let num_connections = min(peer_list.len(), file_hash.pieces.len());
        let mut assembler = FileAssembler::new(file_hash.clone()).await;

        let mut connection_handles = Vec::new();
        //spawn the correct number of connections
        for i in 0..num_connections {
            let mut peer_connection = self.register_new_connection().await?;

            let conn_tx = assembler.get_conn_tx();
            let request_rx = assembler.subscribe_new_connection();
            let peer_id = peer_list[i];
            let handle = tokio::spawn(async move {
                
                let res = peer_connection.requester_connection(peer_id,conn_tx, request_rx).await;
                if res.is_err() {
                    eprintln!("connection error: {}", res.err().unwrap());
                }
            });
            connection_handles.push(handle);
        }

        //begin assemble task
        assembler.send_requests(num_connections, file_hash).await?;

        for handle in connection_handles {
            handle.await?;
        }

        Ok(())
    }

    pub async fn advertise(&self, info_hash: crate::connection::connection::InfoHash) -> Result<ClientId, Box<dyn std::error::Error>> {
        let mut client = self.client.clone();

        //todo make hash active
        let request = Request::new(FileMessage {
            id: Some(self.uid.clone()),
            info_hash: Some(info_hash),
        });

        let resp = client.advertise(request).await?;

        Ok(resp.into_inner())
    }

    async fn advertise_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        let my_hashes = self.file_hashes.read().await.clone().into_values();

        for hash in my_hashes {
            self.advertise(hash).await?;
        }

        Ok(())
    }

    pub async fn get_server_files(&self) -> Result<Vec<InfoHash>, Box<dyn std::error::Error>> {
        let mut server_connection = self.client.clone();

        let res = server_connection.get_all_files(Request::new(())).await?;
        let info_hashes = res.into_inner().info_hashes;

        Ok(info_hashes)

    }

    /// announce is a method used update server with connection details
    /// this will primarily be used by init() and called on a periodic basis
    /// this is essentially a "keep alive" method
    pub async fn announce(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
