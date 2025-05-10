use crate::connection::{turn_client::TurnClient, ClientId, RegisterRequest, TurnPacket, turn_packet::Body, };
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::Status;

pub struct TurnFallback {
    /// channel for sending TurnPackets to your peer
    tx: mpsc::Sender<TurnPacket>,
}

impl TurnFallback {
    /// Call this on the **seeder** side.
    /// - `session_id`: your torrent’s info_hash (or UUID)
    /// - `seeder_id`: your ClientId
    /// - `leecher_id`: the peer’s ClientId
    pub async fn start_seeding(
        mut turn_client: TurnClient<tonic::transport::Channel>,
        session_id: String,
        seeder_id: ClientId,
        leecher_id: ClientId,
    ) -> Result<Self, Status> {
        // 1) Register for incoming Requests from leecher:
        let mut inbound = turn_client
            .register(RegisterRequest {
                session_id: session_id.clone(),
                client_id: Some(seeder_id.clone()),
                is_seeder: true,
            })
            .await?
            .into_inner();

        // 2) Create an outbound channel to send Pieces:
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);
        let outbound = ReceiverStream::new(rx);

        // 3) Spawn a task to receive Requests and stub‐handle them:
        tokio::spawn(async move {
            while let Some(Ok(pkt)) = inbound.next().await {
                if let Some(Body::Request(req)) = pkt.body {
                    // todo handle received requests
                }
            }
        });

        // 4) Spawn a task to pump Pieces into the Send RPC:
        tokio::spawn(async move {
            let _ = turn_client.send(outbound).await;
        });

        Ok(TurnFallback { tx })
    }

    /// Call this on the **leecher** side.
    /// - `session_id`: your torrent’s info_hash (or UUID)
    /// - `leecher_id`: your ClientId
    /// - `seeder_id`: the peer’s ClientId
    pub async fn start_leeching(
        mut turn_client: TurnClient<tonic::transport::Channel>,
        session_id: String,
        leecher_id: ClientId,
        seeder_id: ClientId,
    ) -> Result<Self, Status> {
        // 1) Register for incoming Pieces from seeder:
        let mut inbound = turn_client
            .register(RegisterRequest {
                session_id: session_id.clone(),
                client_id: Some(leecher_id.clone()),
                is_seeder: false,
            })
            .await?
            .into_inner();

        // 2) Spawn a task to receive Pieces and stub‐handle them:
        tokio::spawn(async move {
            while let Some(Ok(pkt)) = inbound.next().await {
                if let Some(Body::Piece(bytes)) = pkt.body {
                    // todo handle received pieces
                }
            }
        });

        // 3) Create an outbound channel to send Requests:
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);
        let outbound = ReceiverStream::new(rx);

        // 4) Spawn a task to pump Requests into the Send RPC:
        tokio::spawn(async move {
            let _ = turn_client.send(outbound).await;
        });

        Ok(TurnFallback { tx })
    }

    /// Utility for either side to send a TurnPacket to its peer:
    pub async fn send(&self, pkt: TurnPacket) -> Result<(), Status> {
        self.tx
            .send(pkt)
            .await
            .map_err(|e| Status::unavailable(format!("failed to send TurnPacket: {}", e)))
    }
}
