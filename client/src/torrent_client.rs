use std::io::{ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};
use stunclient::StunClient;
use tokio::net::{UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;
use tonic::{Request, Response};
use crate::quic_p2p_sender::QuicP2PConn;
use crate::turn_fallback::TurnFallback;
use crate::server_connection::ServerConnection;
use crate::connection::connection::{PeerId, FileMessage, ClientId, PeerList};
use tokio_util::sync::CancellationToken;
use local_ip_address::local_ip;
use tokio::time::{sleep, timeout};

#[derive(Debug)]
pub struct TorrentClient {
    server: ServerConnection,
    pub_socket: Option<UdpSocket>,
    priv_socket: Option<UdpSocket>,
    pub(crate) self_addr: PeerId,
}

impl TorrentClient {

    pub async fn new(server: &mut ServerConnection) -> Result<TorrentClient, Box<dyn std::error::Error>> {

        //bind port and get public facing id
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        let stun_server = "stun.l.google.com:19302".to_socket_addrs().unwrap().filter(|x|x.is_ipv4()).next().unwrap();
        let client = StunClient::new(stun_server);
        let external_addr = client.query_external_address(&socket)?;

        let pub_ipaddr =  match external_addr.ip() {
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
            pub_ipaddr,
            pub_port: external_addr.port() as u32,
            priv_ipaddr: u32::from_be_bytes(priv_ipaddr.octets()),
            priv_port: priv_port as u32,
        };
        socket.set_nonblocking(true)?;
        
        server.register_server_connection(self_addr.clone()).await?;
        
        let server = server.clone();
        
        Ok(
            TorrentClient {
                server,
                pub_socket: Some(UdpSocket::try_from(socket)?) ,
                priv_socket: Some(priv_socket),
                self_addr,
            },
        )
    }

    async fn hole_punch(&mut self, peer_addr: SocketAddr ) -> Result<UdpSocket, Box<dyn std::error::Error + Send + Sync>> {

        //todo maybe don't take this here
        let socket_arc = Arc::new(self.pub_socket.take().unwrap());
        let socket_clone = socket_arc.clone();
        let cancel_token = CancellationToken::new();
        let token_clone = cancel_token.clone();

        let punch_string = b"HELPFUL_SERF";

        println!("Starting Send to peer ip: {}, port: {}", peer_addr.ip(), peer_addr.port());

        let send_task = tokio::spawn(async move {
            for i in 0..50 {
                let res = socket_arc.send_to(punch_string, peer_addr).await;

                if res.is_err() {
                    println!("Send Failed: {}", res.err().unwrap());
                }

                if token_clone.is_cancelled() {
                    println!("Send Cancelled");
                    return Ok(socket_arc);
                }

                sleep(Duration::from_millis(10)).await;
            }
            Err(Box::<dyn std::error::Error + Send + Sync>::from("send task finished without succeeding"))
        });


        let read_task = tokio::spawn( async move {
            let mut recv_buf = [0u8; 1024];
            
            let result = timeout(Duration::from_secs(5), async {
                loop {
                    match socket_clone.recv_from(&mut recv_buf).await {
                        Ok((n, src)) => {
                            println!("Received from {}: {:?}", src, &recv_buf[..n]);
                            if &recv_buf[..n] == punch_string {
                                // println!("Punched SUCCESS {}", src);

                                cancel_token.cancel();
                                drop(socket_clone);
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            eprintln!("Recv error: {}", e);
                            return Err(Box::<dyn std::error::Error + Send + Sync>::from(e));
                        },
                    }
                }
            }).await;
            
            match result {
                Ok(res) => Ok(res),
                Err(e) => Err(e),
            } 
        });

        let socket_arc = match send_task.await {
            Ok(Ok(socket)) => socket,
            Ok(Err(e)) => return Err(e),
            Err(join_err) => return Err(Box::new(join_err)),
        };

        let read_res = read_task.await?;
        match read_res {
            Ok(_) => {
                let socket = Arc::try_unwrap(socket_arc).unwrap();
                println!("Punch Success: {:?}", socket);
                Ok(socket)
            }
            _ => { Err(Box::new(std::io::Error::new(ErrorKind::TimedOut, "hole punch timed out"))) }
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
        let pub_ip_addr = Ipv4Addr::from(peer_id.pub_ipaddr);
        let pub_port = peer_id.pub_port as u16;
        let peer_addr = SocketAddr::from((pub_ip_addr, pub_port));


        println!("peer to send {:?}", peer_id);

        //todo 1. try connection over local NAT
        if self.self_addr.pub_ipaddr == peer_id.pub_ipaddr {
            // println!("Returned value {:?}", socket);
            //start quick server
            let socket = self.priv_socket.take().unwrap();
            let p2p_sender = QuicP2PConn::create_quic_server(self, socket, peer_id, self.server.clone(), Ipv4Addr::from(self.self_addr.priv_ipaddr).to_string()).await?;
            println!("P2P endpoint created successfully");
            p2p_sender.quic_listener().await?;
        }

        //todo 2. try hole punch
        if let Ok(socket) = self.hole_punch(peer_addr).await {
            println!("Returned value {:?}", socket);
            //start quick server
            let p2p_sender = QuicP2PConn::create_quic_server(self, socket, peer_id, self.server.clone(), Ipv4Addr::from(self.self_addr.pub_ipaddr).to_string()).await?;
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
    pub async fn file_request(&self, client_id: ClientId , file_hash: u32) -> Result<PeerList, Box<dyn std::error::Error>> {
        let mut client = self.server.client.clone();
        
        
        let request = Request::new(FileMessage {
            id: Some(client_id),
            info_hash: file_hash,
        });
        
        let resp = client.send_file_request(request).await?;
        
        Ok(resp.into_inner())
    }

    ///Used when client is requesting a file
    pub async fn get_file_from_peer(&mut self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        let ip_addr = Ipv4Addr::from(peer_id.pub_ipaddr);
        let port = peer_id.pub_port as u16;
        let peer_addr = SocketAddr::from((ip_addr, port));
       
        let mut server_connection = self.server.client.clone();
        server_connection.init_cert_sender(self.self_addr).await?;

        //todo refactor for final implementation
        if self.self_addr.pub_ipaddr == peer_id.pub_ipaddr {
            let ip_addr = Ipv4Addr::from(peer_id.priv_ipaddr);
            let port = peer_id.priv_port as u16;
            let lan_peer_addr = SocketAddr::from((ip_addr, port));
            let priv_socket = self.priv_socket.take().unwrap();

            let p2p_conn = QuicP2PConn::create_quic_client(priv_socket, self.self_addr, self.server.clone()).await?;
            p2p_conn.connect_to_peer_server(lan_peer_addr).await?;
        }

            //init the map so cert can be retrieved

        if let Ok(socket) = self.hole_punch(peer_addr).await {
            let ip_addr = Ipv4Addr::from(peer_id.pub_ipaddr);
            let port = peer_id.pub_port as u16;
            let peer_addr = SocketAddr::from((ip_addr, port));

            let p2p_conn = QuicP2PConn::create_quic_client(socket, self.self_addr, self.server.clone()).await?;
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
        let request = Request::new(FileMessage {
            id: Some(client_id),
            info_hash: 12345,
        });
        
        let resp = client.advertise(request).await?;
        
        Ok(resp.into_inner())
    }
}