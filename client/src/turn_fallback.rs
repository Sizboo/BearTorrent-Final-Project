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
        seeder_id: ClientId,
        leecher_id: ClientId,
    ) -> Result<Self, Status> {
        let session_id   = make_session_id(seeder_id.uid, leecher_id.uid);

        // 1) Register for incoming Requests from leecher:
        let mut inbound = turn_client
            .register(RegisterRequest {
                session_id: session_id.clone(),
                client_id: leecher_id.clone(),
                is_seeder: true,
            })
            .await?
            .into_inner();

        // 2) Create an outbound channel to send Pieces:
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);
        let outbound = ReceiverStream::new(rx);

        let tx_for_requests = tx.clone();

        // 3) Spawn a task to receive Requests and stub‐handle them:
        tokio::spawn(async move {
            while let Some(Ok(pkt)) = inbound.next().await {
                if let Some(Body::Request(req)) = pkt.body {

                    // todo get actual values for payload and index
                    let payload = [].to_vec();
                    let index = 0;

                    let reply = TurnPacket {
                        session_id: session_id.clone(),
                        target_id: leecher_id.clone(),
                        body: Some(Body::Piece(TurnPiece {
                            payload,
                            index,
                        })),
                    };

                    if let Err(e) = tx_for_requests.send(reply).await {
                        eprintln!("failed to send piece over TURN: {}", e);
                    }
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
        leecher_id: ClientId,
        seeder_id: ClientId,
    ) -> Result<Self, Status> {
        let session_id   = make_session_id(seeder_id.uid, leecher_id.uid);

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
                if let Some(Body::Piece(tp)) = pkt.body {
                    let index  = tp.index;
                    let payload = tp.payload;

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

fn make_session_id(a: &str, b: &str) -> String {
    let mut pair = [a, b];
    pair.sort_unstable();
    pair.join(":")
}