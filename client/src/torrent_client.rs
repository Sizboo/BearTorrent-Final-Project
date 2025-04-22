use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use stunclient::StunClient;
use std::net::UdpSocket;
use tokio::net::UdpSocket as TokioUdpSocket;
use std::sync::Arc;
use tonic::Request;
use tonic::transport::{Channel};
use connection::{PeerId, connector_client::ConnectorClient};
use crate::connection::FileMessage;

pub mod connection {
    tonic::include_proto!("connection");
}

#[derive(Debug)]
pub struct TorrentClient {
    client: ConnectorClient<Channel>,
    socket: TokioUdpSocket,
    self_addr: PeerId,
}

impl TorrentClient {

    pub async fn new(channel: Channel) -> Result<TorrentClient, Box<dyn std::error::Error>> {
        
        //bind port and get public facing id
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        let stun_server = "stun.l.google.com:19302".to_socket_addrs().unwrap().filter(|x|x.is_ipv4()).next().unwrap();
        let client = StunClient::new(stun_server);
        let external_addr = client.query_external_address(&socket)?;

        let ipaddr =  match external_addr.ip() {
            IpAddr::V4(v4) => Ok(u32::from_be_bytes(v4.octets())),
            IpAddr::V6(_) => Err("Cannot convert IPv6 to u32"),
        }?;
       
        let self_addr = PeerId {
            ipaddr,
            port: external_addr.port() as u32,
        };
        
        Ok(
            TorrentClient {
                client: ConnectorClient::new(channel),
                socket: TokioUdpSocket::from_std(socket)?,
                self_addr,
            }
        )
    }

    ///init is used to initialize and maintain a connection to the torrent server
    /// init MUST be called
    /// it will need to initialize a connection and send a periodic announcement so server knows it is an available seeder
    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        Ok(())
    }
    
    async fn hole_punch(&self, peer_id: PeerId ) -> Result<(), Box<dyn std::error::Error>> {
        let ip_addr = Ipv4Addr::from(peer_id.ipaddr);
        let port = peer_id.port as u16;
        let peer_addr = SocketAddr::from((ip_addr, port));

        println!("starting to send udp packets from {:?} to {:}", self.self_addr, peer_addr);
        for _ in 0 ..10 {
            let _ = self.socket.try_send_to(b"whatup dawg", peer_addr);
        }
        
        let mut recv_buf = [0u8; 1024];
        if let Ok((n, src)) = self.socket.recv_from(&mut recv_buf).await {
            println!("Received a message from {}: {:?}", src, std::str::from_utf8(&recv_buf[..n]));
        } else {
            println!("No response")
        }

        Ok(())
    }

    /// send_data is used to send data to peer
    /// Process:
    /// parameters - peer ip and port
    /// return Ok() upon completion Err() if connection cannot be made or data send fail
    ///
    /// it will need to try UDP hole punching with the given parameter
    /// then send data
    pub async fn send_data(self: Arc<Self>, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {

       //todo actual send logic to other per
        self.hole_punch(peer_id).await?;

        Ok(())
    }

    ///seeding is used as a listening process to begin sending data upon request
    /// it simply awaits a server request for it to send data
    pub async fn seeding(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = self.client.clone();
        let self_addr = self.self_addr.clone();
        loop {
            // calls get_peer
            let response = client.get_peer(self_addr).await;
            println!("response from get_peer: {:?}", response);

            // waits for response from get_peer
            match response {
                Ok(res) => {
                    let peer_id = res.into_inner();
                    
                    println!("peer to send {:?}", peer_id);
                    
                    let self_clone = Arc::clone(&self);
                    
                    tokio::spawn(async move {
                        self_clone.send_data(peer_id).await.unwrap();
                    });
                }
                Err(e) => {
                    eprintln!("Failed to get peer: {:?}", e);
                }
            }
        }
    }

    ///request is a method used to request necessary connection details from the server
    pub async fn file_request(&self, file_hash: u32) -> Result<connection::PeerList, Box<dyn std::error::Error>> {
        let mut client = self.client.clone();
        
        
        let request = Request::new(connection::FileMessage {
            id: Some(self.self_addr),
            info_hash: file_hash,
        });
        
        let resp = client.send_file_request(request).await?;
        
        Ok(resp.into_inner())
    }
   
    ///Used when client is requesting a file
    pub async fn get_file_from_peer(&self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {
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
    
    pub async fn advertise(self: Arc<Self>) -> Result<PeerId, Box<dyn std::error::Error>> {
        let mut client = self.client.clone();
        
        //todo make hash active
        let request = Request::new(connection::FileMessage {
            id: Some(self.self_addr),
            info_hash: 12345,
        });
        
        let resp = client.advertise(request).await?;
        
        Ok(resp.into_inner())
    }
    

}
