use std::io::{ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use stunclient::StunClient;
use tokio::net::{UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::{time::{sleep, timeout}, sync::mpsc};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{Request, Response, transport::Channel};
use connection::{PeerId, ClientId, FileMessage, turn_client::TurnClient, TurnPacket};
use crate::quic_p2p_sender::QuicP2PConn;
use crate::turn_fallback::TurnFallback;
use crate::server_connection::ServerConnection;
use tokio_util::sync::CancellationToken;

pub mod connection {
    tonic::include_proto!("connection");
}

#[derive(Debug)]
pub struct TorrentClient {
    server: ServerConnection,
    socket: Option<UdpSocket>,
    pub(crate) self_addr: PeerId,
}

impl TorrentClient {

    pub async fn new(server: &mut ServerConnection) -> Result<TorrentClient, Box<dyn std::error::Error>> {

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
        
        server.register_server_connection(self_addr.clone()).await?;
        
        let server = server.clone();
        
        Ok(
            TorrentClient {
                server,
                socket: Some(UdpSocket::try_from(socket)?) ,
                self_addr,
            },
        )
    }


    async fn hole_punch(&mut self, peer_addr: SocketAddr ) -> Result<UdpSocket, Box<dyn std::error::Error + Send + Sync>> {
        let socket = Arc::new(self.socket.take().unwrap());
        let cancel = CancellationToken::new();

        println!("Starting Send to peer ip: {}, port: {}", peer_addr.ip(), peer_addr.port());

        {
            let s_send = socket.clone();
            let c_send = cancel.clone();
            tokio::spawn(async move {
                for _ in 0..50 {
                    if let Err(e) = s_send.send_to(b"HELPFUL_SERF", peer_addr).await {
                        eprintln!("hole_punch send error: {}", e);
                    }
                    // if we get cancelled by the receive, stop sending early
                    if c_send.is_cancelled() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                // once this loop finishes or is cancelled, task simply ends
            });
        }

        let recv_task = {
            let s_recv = socket.clone();
            let c_recv = cancel.clone();
            async move {
                let mut buf = [0u8; 1024];
                loop {
                    let (n, src) = s_recv.recv_from(&mut buf).await?;
                    if &buf[..n] == b"HELPFUL_SERF" {
                        // signal send‐task to stop
                        c_recv.cancel();
                        drop(s_recv);
                        return Ok(()) as Result<(), Box<dyn std::error::Error + Send + Sync>>;
                    }
                }
            }
        };

        match tokio::time::timeout(Duration::from_secs(5), recv_task).await {
            Ok(Ok(())) => {
                // got a successful punch
                let socket = Arc::try_unwrap(socket).unwrap();
                println!("Punch succeeded!");
                Ok(socket)
            }
            Ok(Err(e)) => {
                Err(e)
            }
            Err(_) => {
                // receiving loop timed out
                eprintln!("hole punch timed out, falling back to TURN");
                Err(Box::new(std::io::Error::new(
                    ErrorKind::TimedOut,
                    "hole punch timed out",
                )))
            }
        }
    }

    ///seeding is used as a listening process to begin sending data upon request
    /// it simply awaits a server request for it to send data
    pub async fn seeding(server: &mut ServerConnection) -> Result<(), Box<dyn std::error::Error>> {
        
        loop {
            let mut torrent_client = TorrentClient::new(server).await?;

            let mut server_client = torrent_client.server.client.clone();

            //todo REMOVE THIS and add proper ERROR CHECKING for REAL USE
            let _ = server_client.advertise(FileMessage {
                id: torrent_client.server.uid.clone(),
                info_hash: 1234,
            }).await;

            // calls get_peer
            //todo remove unwarp error checking IS IMPORTANT HERE
            let uid = torrent_client.server.uid.clone().unwrap();
            println!("My Client ID: {:?}", uid);
            let response = server_client.get_peer(uid).await;

            // waits for response from get_peer
            match response {
                Ok(res) => {

                    let res = torrent_client.connect_to_peer(res).await;
                    if res.is_err() {
                        println!("Connect Failed: {}", res.err().unwrap());
                    }

                }
                Err(e) => {
                    eprintln!("Failed to get peer: {:?}", e);
                }
            }
        }
    }
    
    pub async fn connect_to_peer(&mut self, res: Response<PeerId>) -> Result<(), Box<dyn std::error::Error>> {
        let peer_id = res.into_inner();

        let ip_addr = Ipv4Addr::from(peer_id.ipaddr);
        let port = peer_id.port as u16;
        let peer_addr = SocketAddr::from((ip_addr, port));
        
        println!("peer to send {:?}", peer_id);

        //todo 1. try connection over local NAT


        //todo 2. try hole punch

        if let Ok(socket) = self.hole_punch(peer_addr).await {
            println!("Returned value {:?}", socket);
            //start quick server
            let p2p_sender = QuicP2PConn::create_quic_server(self, socket, peer_id, self.server.clone()).await?;
            p2p_sender.quic_listener().await?;
        } else {
            // TURN for sending here
            let client_id = self.server.uid.clone()
                .expect("server.uid must be set before calling TurnFallback::start");

            let fallback = TurnFallback::start(self.server.turn.clone(), client_id).await?;

            let response = self.server.client.get_client_id(peer_id).await?;
            let target = response.into_inner();
            let buf = "data sent over TURN".as_bytes().to_vec();

            tokio::time::sleep(Duration::from_millis(1000)).await;
            fallback.send_to(target, buf).await?;
        }

        Ok(())
    }

    ///request is a method used to request necessary connection details from the server
    pub async fn file_request(&self, client_id: ClientId , file_hash: u32) -> Result<connection::PeerList, Box<dyn std::error::Error>> {
        let mut client = self.server.client.clone();
        
        
        let request = Request::new(connection::FileMessage {
            id: Some(client_id),
            info_hash: file_hash,
        });
        
        let resp = client.send_file_request(request).await?;
        
        Ok(resp.into_inner())
    }

    ///Used when client is requesting a file
    pub async fn get_file_from_peer(&mut self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        let ip_addr = Ipv4Addr::from(peer_id.ipaddr);
        let port = peer_id.port as u16;
        let peer_addr = SocketAddr::from((ip_addr, port));
       
        //todo refactor for final implementation 
        
        //init the map so cert can be retrieved 
        let mut server_connection = self.server.client.clone();
        server_connection.init_cert_sender(self.self_addr).await?;


        if let Ok(socket) = self.hole_punch(peer_addr).await {
            let ip_addr = Ipv4Addr::from(peer_id.ipaddr);
            let port = peer_id.port as u16;
            let peer_addr = SocketAddr::from((ip_addr, port));

            let p2p_conn = QuicP2PConn::create_quic_client(socket, self.self_addr,self.server.clone()).await?;
            p2p_conn.connect_to_peer_server(peer_addr).await?;
        } else {
            // TURN for receiving here
            let client_id = self.server.uid.clone()
                .expect("server.uid must be set before calling TurnFallback::start");

            let fallback = TurnFallback::start(self.server.turn.clone(), client_id).await?;

            // TODO remove... just needed to have this to keep the program open long enough to receive data
            tokio::time::sleep(Duration::from_millis(10000)).await;
        }

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
    
    pub async fn advertise(&self, client_id: ClientId) -> Result<ClientId, Box<dyn std::error::Error>> {
        let mut client = self.server.client.clone();
        
        //todo make hash active
        let request = Request::new(connection::FileMessage {
            id: Some(client_id),
            info_hash: 12345,
        });
        
        let resp = client.advertise(request).await?;
        
        Ok(resp.into_inner())
    }
}