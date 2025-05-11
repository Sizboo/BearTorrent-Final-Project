use std::io::{ErrorKind};
use std::net::{Ipv4Addr, SocketAddr};
use tokio::{net::UdpSocket, sync::mpsc};
use std::sync::Arc;
use std::time::Duration;
use tonic::{Response};
use crate::quic_p2p_sender::QuicP2PConn;
use crate::torrent_client::TorrentClient;
use crate::connection::connection::{PeerId, FullId, ClientId};
use tokio_util::sync::CancellationToken;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};
use crate::message::Message;
use crate::turn_fallback::TurnFallback;

#[derive(Debug)]
pub struct PeerConnection {
    pub(crate) server: TorrentClient,
    pub(crate) pub_socket: Option<UdpSocket>,
    pub(crate) priv_socket: Option<UdpSocket>,
    pub(crate) self_addr: PeerId,
    // peer_conns: Vec<PeerConnection>,
}

impl PeerConnection {

    async fn hole_punch(&mut self, peer_addr: SocketAddr ) -> Result<UdpSocket, Box<dyn std::error::Error + Send + Sync>> {

        //todo maybe don't take this here
        let socket_arc = Arc::new(self.pub_socket.take().unwrap());
        let socket_clone = socket_arc.clone();
        let cancel_token = CancellationToken::new();
        let token_clone = cancel_token.clone();

        let punch_string = b"HELPFUL_SERF";

        println!("Starting Send to peer ip: {}, port: {}", peer_addr.ip(), peer_addr.port());

        let send_task = tokio::spawn(async move {
            for i in 0..200 {
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


    pub async fn seeder_connection(&mut self, res: Response<PeerId>) -> Result<(), Box<dyn std::error::Error>> {

        let mut server_connection = self.server.client.clone();
        let hole_punch_handle = server_connection.await_hole_punch_trigger(self.self_addr.clone());


        let peer_id = res.into_inner();
        let pub_ip_addr = Ipv4Addr::from(peer_id.ipaddr);
        let pub_port = peer_id.port as u16;
        let peer_addr = SocketAddr::from((pub_ip_addr, pub_port));

        // create a PeerConnection and get the receiver

        println!("peer to send {:?}", peer_id);


        // 1. try connection over local NAT
        if self.self_addr.ipaddr == peer_id.ipaddr {
            //start quick server
            let socket = self.priv_socket.take().unwrap();
            let mut p2p_sender = QuicP2PConn::create_quic_server(
                socket,
                peer_id,
                self.server.clone(),
                Ipv4Addr::from(self.self_addr.priv_ipaddr).to_string(),
            ).await?;
            // println!("P2P quic endpoint created successfully");
            let res = p2p_sender.quic_listener(self.server.file_hashes.clone()).await;
            match res {
                Ok(()) => {
                    println!("SEEDER: Quic connection within LAN success!");
                    // conn_success = true
                    return Ok(());
                },
                Err(e) => {
                    println!("SEEDER: LAN based quic connection failed\n {:?}", e);
                }
            }
        }

        //2. try connection across NAT
        {
            let timeout_duration = Duration::from_secs(5);
            let res = timeout(timeout_duration, hole_punch_handle).await?;

            match res {
                Ok(_) => {
                    println!("Seeder got hole punch notif");
                    if let Ok(socket) = self.hole_punch(peer_addr).await {
                        // println!("Returned value {:?}", socket);
                        //start quick server


                        let mut p2p_sender = QuicP2PConn::create_quic_server(
                            socket,
                            peer_id,
                            self.server.clone(),
                            Ipv4Addr::from(self.self_addr.ipaddr).to_string(),
                        ).await?;
                        // println!("SEEDER: P2P quic endpoint across NAT created successfully");
                        match p2p_sender.quic_listener(self.server.file_hashes.clone()).await {
                            Ok(()) => {
                                println!("SEEDER: Quic connection across NAT successful!");
                                return Ok(())
                            },
                            Err(e) => {
                                println!("SEEDER: Connection across NAT after hole punch failed");
                            }
                        }
                    }
                },
                Err(e) => {
                    println!("SEEDER: Failed to receive hole punch trigger");
                }
            }
        }
            
        // Fall back connection on TURN
        {
            println!("Trying to seed over TURN...");
            // TURN for sending here
            TurnFallback::start_seeding(self.server.turn.clone(), self.self_addr, peer_id, self.server.file_hashes.clone()).await;
        }

        Ok(())
    }



    ///Used when client is requesting a file
    pub async fn requester_connection(&mut self, peer_id: PeerId, conn_tx: mpsc::Sender<Message>, request_rx:  mpsc::Receiver<Message> ) -> Result<(), Box<dyn std::error::Error>> {
        
        //init the map so cert can be retrieved
        let mut server_connection = self.server.client.clone();
        server_connection.send_file_request(FullId {
            self_id: Some(self.server.uid.clone()),
            peer_id: Some(peer_id.clone()),
        }).await?;
        
        
        server_connection.init_cert_sender(self.self_addr).await?;


        let conn_rx = Arc::new(Mutex::new(request_rx));

        if self.self_addr.ipaddr == peer_id.ipaddr {
            let ip_addr = Ipv4Addr::from(peer_id.priv_ipaddr);
            let port = peer_id.priv_port as u16;
            let lan_peer_addr = SocketAddr::from((ip_addr, port));
            let priv_socket = self.priv_socket.take().unwrap();

            let mut p2p_conn = QuicP2PConn::create_quic_client(
                priv_socket,
                self.self_addr,
                self.server.clone(),
            ).await?;


            match p2p_conn.connect_to_peer_server(lan_peer_addr, conn_tx.clone(), conn_rx.clone()).await {
                Ok(()) => {
                    println!("REQUESTER: successful connection within LAN");
                    return Ok(())
                },
                Err(_) => {
                    println!("REQUESTER: connect over LAN failed");
                }
            }
        }

        {
            println!("In hole punch");
            let ip_addr = Ipv4Addr::from(peer_id.ipaddr);
            let port = peer_id.port as u16;
            let peer_addr = SocketAddr::from((ip_addr, port));

            //add pause to give other peer time to wait on notify handle
            sleep(Duration::from_millis(1000)).await;

            //initiate hole punch routine with other peer
            println!("PeerId {:?}", peer_id);
            server_connection.init_punch(peer_id).await?;

            sleep(Duration::from_millis(250)).await;

            if let Ok(socket) = self.hole_punch(peer_addr).await {
                let ip_addr = Ipv4Addr::from(peer_id.ipaddr);
                let port = peer_id.port as u16;
                let peer_addr = SocketAddr::from((ip_addr, port));

                let mut p2p_conn = QuicP2PConn::create_quic_client(
                    socket,
                    self.self_addr,
                    self.server.clone(),
                ).await?;

                match p2p_conn.connect_to_peer_server(peer_addr, conn_tx.clone(), conn_rx.clone()).await {
                    Ok(()) => {
                        println!("REQUESTER: successful connection across NAT");
                        return Ok(())
                    },
                    Err(_) => {
                        println!("REQUESTER: connect across NAT failed");
                    }
                }
            }
        }
            
        
        {
            // TURN for receiving here
            println!("Trying to leech over TURN...");
            TurnFallback::start_leeching(self.server.turn.clone(), self.self_addr, peer_id, conn_tx, conn_rx).await;

            // // TODO remove... just needed to have this to keep the program open long enough to receive data
            // tokio::time::sleep(Duration::from_secs(5)).await;
        }

        Ok(())
    }


    

}