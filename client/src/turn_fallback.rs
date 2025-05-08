use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{Request, transport::Channel};
use crate::connection::connection::{ClientId, TurnPacket, turn_client};
use crate::message::Message;


// we will use this to manage TURN if we need it as a fallback
pub struct TurnFallback {
    self_id: ClientId,
    tx: mpsc::Sender<TurnPacket>,
}

impl TurnFallback {
    pub async fn start(
        mut turn_client: turn_client::TurnClient<Channel>, // TorrentClient.server.turn
        self_id: ClientId,
        conn_tx: mpsc::Sender<Message>,
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

        // get our inbound data stream
        let resp = turn_client.relay(Request::new(outbound)).await?;
        let mut inbound = resp.into_inner();

        tokio::spawn(async move {
            while let Some(Ok(pkt)) = inbound.next().await {
                let from = pkt.client_id.unwrap();

                // send received data off to our connection
                // todo make sure this works
                conn_tx.send(pkt.payload).await;
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