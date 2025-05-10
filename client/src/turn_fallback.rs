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
    /// function to start seeding via TURN
    pub async fn start_seeding(
        mut turn_client: TurnClient<tonic::transport::Channel>,
        seeder_id: ClientId,
        leecher_id: ClientId,
        file_map: Arc<RwLock<HashMap<[u8; 20], InfoHash>>>,
    ) -> Result<(), Status> {
        println!("made it to start_seeding");
        let session_id = make_session_id(&seeder_id.uid, &leecher_id.uid);

        // register for turn as the seeder
        let mut inbound = turn_client
            .register(RegisterRequest {
                session_id: session_id.clone(),
                client_id: Some(leecher_id.clone()),
                is_seeder: true,
            })
            .await?
            .into_inner();

        println!("made it past seeding register");

        // create channel to send pieces thru
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);
        let outbound = ReceiverStream::new(rx);


        let session_id_clone = session_id.clone();

        // todo maybe refactor this it feels weird now that it's actually implemented but oh well it's late and I don't feel like doing it at the moment
        // spawns the sending task
        let mut req = tonic::Request::new(outbound);
        println!("made it past sending req");

        // attach metadata to the stream for registering with the turn service
        let md = req.metadata_mut();
        md.insert("x-session-id", session_id_clone.parse().unwrap());
        md.insert("x-client-id", seeder_id.uid.parse().unwrap());
        md.insert("x-role", "seeder".parse().unwrap());

        println!("made it past sending metadata");

        if let Err(e) = turn_client.send(req).await {
            eprintln!("TURN Send stream ended unexpectedly: {}", e);
        }


        // main loop to receive requests and send pieces back
        tokio::spawn(async move {
            println!("made it to Seeder loop");
            while let Some(Ok(pkt)) = inbound.next().await {
                if let Some(Body::Request(req)) = pkt.body {

                    let index: u32 = req.index;

                    // turn the request's hash (bytes in the proto file ik yuck) into the msg form
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
                    println!("sending piece: {:}", index);

                    if let Err(e) = tx.send(reply).await {
                        eprintln!("failed to send piece over TURN: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// function to start leeching via TURN
    pub async fn start_leeching(
        mut turn_client: TurnClient<tonic::transport::Channel>,
        leecher_id: ClientId,
        seeder_id: ClientId,
        conn_tx: mpsc::Sender<Message>,
        conn_rx: Arc<Mutex<mpsc::Receiver<Message>>>,
    ) -> Result<(), Status> {
        println!("made it to start_leeching");
        let session_id = make_session_id(&seeder_id.uid, &leecher_id.uid);

        // register for turn as the leecher
        let mut inbound = turn_client
            .register(RegisterRequest {
                session_id: session_id.clone(),
                client_id: Some(leecher_id.clone()),
                is_seeder: false,
            })
            .await?
            .into_inner();

        println!("made it past leeching register");

        // create channel to send requests thru
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);
        let outbound = ReceiverStream::new(rx);

        let session_id_clone = session_id.clone();

        // spawns the actual sending task
        let mut req = tonic::Request::new(outbound);
        println!("made it past leeching req");

        // attach metadata to the stream for registering with the turn service
        let md = req.metadata_mut();
        md.insert("x-session-id", session_id_clone.parse().unwrap());
        md.insert("x-client-id",  seeder_id.uid.parse().unwrap());
        md.insert("x-role", "leecher".parse().unwrap());

        println!("made it past leeching metadata");

        if let Err(e) = turn_client.send(req).await {
            eprintln!("TURN Send stream ended unexpectedly: {}", e);
        }

        // spawn a task to both receive pieces and requests and process them
        let conn_rx = Arc::clone(&conn_rx);
        println!("made it to Leecher loop");
        loop {
            tokio::select! {
                turn_packet = inbound.next() => {
                    match turn_packet {
                        Some(Ok(pkt)) => {
                            if let Some(Body::Piece(tp)) = pkt.body {

                                let piece_msg = Message::Piece {
                                    index: tp.index,
                                    piece: tp.payload,
                                };

                                conn_tx.send(piece_msg).await;
                            }
                        }
                        _ => continue,
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
                            println!("requested piece {}", index);
                        }
                        None => continue,
                    }
                }
            }
        }

        Ok(())
    }
}


/// function to join 2 ClientIds to make a string for use as a session id for turn
fn make_session_id(a: &str, b: &str) -> String {
    let mut pair = [a, b];
    pair.sort_unstable();
    pair.join(":")
}