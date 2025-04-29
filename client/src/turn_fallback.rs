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

// we will use this to manage TURN if we need it as a fallback
pub struct TurnFallback {
    self_id: ClientId,
    tx: mpsc::Sender<TurnPacket>,
}

impl TurnFallback {
    pub async fn start(
        mut client: TurnClient<Channel>, // TorrentClient.server.turn
        self_id: ClientId,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);
        let outbound = ReceiverStream::new(rx);

        // send our initial packet so we are registered with the TurnService
        let init = TurnPacket {
            client_id: Some(self_id.clone()),
            target_id: None,
            payload: Vec::new(),
        };
        tx.send(init).await?;

        // by the nature of streams, it is easier to set up bidirectional communication and have
        // both sending and receiving use the same relay setup code...
        // it might be helpful later on... or not idk, but we have it lol

        // get our inbound data stream
        let mut resp = client.relay(Request::new(outbound)).await?;
        let mut inbound = resp.into_inner();

        tokio::spawn(async move {
            while let Some(Ok(pkt)) = inbound.next().await {
                let from = pkt.client_id.unwrap();

                // TODO pass off pkt.data to data handler when we make one
                let payload = String::from_utf8_lossy(&pkt.payload);
                println!("from = {}, payload = {}", from.uid, payload);
            }
        });

        // return our TurnFallback so we can send through it
        Ok(TurnFallback { self_id, tx })
    }

    pub async fn send_to(
        &self,
        target_id: ClientId,
        buf: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {

        // assemble packet
        let pkt = TurnPacket {
            client_id: Some(self.self_id.clone()),
            target_id: Some(target_id),
            payload: buf,
        };

        // send via out sending stream
        self.tx.send(pkt).await?;
        Ok(())
    }
}