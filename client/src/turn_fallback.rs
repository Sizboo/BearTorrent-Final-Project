use crate::connection::connection::{turn_client::TurnClient, ClientId, RegisterRequest, TurnPacket,
                                    turn_packet::Body, TurnPiece, TurnPieceRequest, InfoHash};
use crate::message::Message;
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::Status;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::collections::HashMap;
use crate::file_handler::{read_piece_from_file };

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
        file_map: Arc<RwLock<HashMap<[u8; 20], InfoHash>>>,
    ) -> Result<(), Status> {
        let session_id   = make_session_id(&seeder_id.uid, &leecher_id.uid);

        // 1) Register for incoming Requests from leecher:
        let mut inbound = turn_client
            .register(RegisterRequest {
                session_id: session_id.clone(),
                client_id: Some(leecher_id.clone()),
                is_seeder: true,
            })
            .await?
            .into_inner();

        // 2) Create an outbound channel to send Pieces:
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);
        let outbound = ReceiverStream::new(rx);


        let _ = turn_client.send(outbound).await;

        // 3) Spawn a task to receive Requests and stub‐handle them:
        tokio::spawn(async move {
            while let Some(Ok(pkt)) = inbound.next().await {
                if let Some(Body::Request(req)) = pkt.body {

                    let index: u32 = req.index;

                    let hash: [u8; 20] = req
                        .hash
                        .as_slice()
                        .try_into()
                        .expect("hash was not 20 bytes");

                    let info_hash = match file_map.read().await.get(&hash).cloned() {
                        Some(h) => h,
                        None => {
                            eprintln!("missing file info for hash {:?}", hash);
                            return;
                        }
                    };

                    let piece = match read_piece_from_file(info_hash, index) {
                        Ok(vec) => vec,
                        Err(e) => {
                            eprintln!("failed to read piece: {}", e);
                            return;
                        }
                    };

                    let reply = TurnPacket {
                        session_id: session_id.clone(),
                        target_id: Some(leecher_id.clone()),
                        body: Some(Body::Piece(TurnPiece {
                            payload: piece,
                            index,
                        })),
                    };

                    if let Err(e) = tx.send(reply).await {
                        eprintln!("failed to send piece over TURN: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Call this on the **leecher** side.
    /// - `session_id`: your torrent’s info_hash (or UUID)
    /// - `leecher_id`: your ClientId
    /// - `seeder_id`: the peer’s ClientId
    pub async fn start_leeching(
        mut turn_client: TurnClient<tonic::transport::Channel>,
        leecher_id: ClientId,
        seeder_id: ClientId,
        conn_tx: mpsc::Sender<Message>,
        conn_rx: Arc<Mutex<mpsc::Receiver<Message>>>,
    ) -> Result<(), Status> {
        let session_id   = make_session_id(&seeder_id.uid, &leecher_id.uid);

        // 1) Register for incoming Pieces from seeder:
        let mut inbound = turn_client
            .register(RegisterRequest {
                session_id: session_id.clone(),
                client_id: Some(leecher_id.clone()),
                is_seeder: false,
            })
            .await?
            .into_inner();

        // 3) Create an outbound channel to send Requests:
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);
        let outbound = ReceiverStream::new(rx);

        // 4) Spawn a task to pump Requests into the Send RPC:
        tokio::spawn(async move {
            let _ = turn_client.send(outbound).await;
        });

        // 2) Spawn a task to receive Pieces and Requests
        let conn_rx = Arc::clone(&conn_rx);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    turn_packet = inbound.next() => {
                        match turn_packet {
                            Some(Ok(pkt)) => {
                                if let Some(Body::Piece(tp)) = pkt.body {
                                    let index = tp.index;
                                    let payload = tp.payload;
                                    // TODO: handle received pieces
                                }
                            }
                            _ => break,
                        }
                    }

                    request_message = async {
                        let mut rx = conn_rx.lock().await;
                        rx.recv().await
                    } => {
                        match request_message {
                            Some(req) => {
                                let (index, hash) = if let Message::Request { index, hash, .. } = req {
                                    (index, hash)
                                } else {
                                    continue;
                                };

                                let request_packet = TurnPacket {
                                    session_id: session_id.clone(),
                                    target_id: Some(leecher_id.clone()),
                                    body: Some(Body::Request(TurnPieceRequest {
                                        hash: hash.to_vec(),
                                        index,
                                    })),
                                };
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        Ok(())
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