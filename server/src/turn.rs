use std::net::{IpAddr, SocketAddr, Ipv4Addr};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tokio::net::UdpSocket;
use crate::connection::turn_server::Turn;
use crate::connection::TurnPair;

pub struct TurnService {}

impl TurnService {
    async fn relay_loop(
        socket: Arc<UdpSocket>,
        from: SocketAddr,
        to: SocketAddr,
    ) {
        let mut buf = [0u8; 1500];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, src)) => {
                    if src == from {
                        if let Err(e) = socket.send_to(&buf[..n], to).await {
                            eprintln!("Error sending to {}: {}", to, e);
                        }
                    } else {
                        eprintln!("Unexpected source: expected {}, got {}", from, src);
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving from {}: {}", from, e);
                    break;
                }
            }
        }
    }
}

#[tonic::async_trait]
impl Turn for TurnService {
    async fn establish_relay(
        &self,
        request: Request<TurnPair>,
    ) -> Result<Response<()>, Status> {
        let pair = request.into_inner();

        let sender = pair.sender.ok_or_else(|| Status::invalid_argument("missing sender"))?;
        let receiver = pair.receiver.ok_or_else(|| Status::invalid_argument("missing receiver"))?;

        let sender_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from(sender.ipaddr)), sender.port as u16);
        let receiver_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from(receiver.ipaddr)), receiver.port as u16);

        let socket = UdpSocket::bind("0.0.0.0:0").await
            .map_err(|e| Status::internal(format!("Socket bind failed: {e}")))?;

        let socket = Arc::new(socket);
        let socket_sender = socket.clone();

        tokio::spawn(TurnService::relay_loop(socket_sender, sender_addr, receiver_addr));

        Ok(Response::new(()))
    }
}