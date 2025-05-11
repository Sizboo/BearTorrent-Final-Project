use crate::connection::connection::{turn_client::TurnClient, ClientId, RegisterRequest, TurnPacket,
                                    turn_packet::Body, TurnPiece, TurnPieceRequest, InfoHash};
use crate::message::Message;
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::Status;
use std::sync::Arc;
use tokio::{sync::{Mutex, RwLock}, time::{sleep, Duration}};
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

        // 1) Register for TURN as the seeder (this is a unary→stream RPC)
        let mut inbound = turn_client
            .register(RegisterRequest {
                session_id: session_id.clone(),
                is_seeder: true,
            })
            .await?
            .into_inner();

        println!("made it past seeding register");

        // 2) Create your local channel and wrap it in a ReceiverStream
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);
        let outbound = ReceiverStream::new(rx);

        // 3) Build the Request with metadata
        let mut req = tonic::Request::new(outbound);
        println!("made it past sending req");
        let md = req.metadata_mut();
        md.insert("x-session-id", session_id.parse().unwrap());
        md.insert("x-client-id",   seeder_id.uid.parse().unwrap());
        md.insert("x-role",        "seeder".parse().unwrap());
        println!("made it past sending metadata");

        // 4) Spawn the send() future so the TURN send‐stream stays open
        let mut client_clone = turn_client.clone();
        tokio::spawn(async move {
            match client_clone.send(req).await {
                Ok(_) => eprintln!("TURN send RPC ended normally"),
                Err(e) => eprintln!("TURN send stream ended unexpectedly: {}", e),
            }
        });

        // 5) Now spawn your seeder‐loop to read inbound piece‐requests and reply
        tokio::spawn({
            let mut inbound = inbound;
            let file_map = file_map.clone();
            let session_id = session_id.clone();
            let leecher_id = leecher_id.clone();
            let mut tx = tx.clone();

            async move {
                println!("made it to Seeder loop");
                loop {
                    match inbound.next().await {
                        Some(Ok(pkt)) => {
                            println!("received {:?}", pkt);
                            if let Some(Body::Request(req)) = pkt.body {
                                let index: u32 = req.index;
                                println!("received request of index: {}", index);

                                // turn proto‐bytes into a fixed [u8;20]
                                let hash: [u8; 20] = req
                                    .hash
                                    .as_slice()
                                    .try_into()
                                    .expect("hash was not 20 bytes");

                                // lookup the file
                                let info_hash = match file_map.read().await.get(&hash).cloned() {
                                    Some(h) => h,
                                    None => {
                                        eprintln!("missing file info for hash {:?}", hash);
                                        return;
                                    }
                                };

                                // load the piece
                                let piece = match read_piece_from_file(info_hash, index) {
                                    Ok(vec) => vec,
                                    Err(e) => {
                                        eprintln!("failed to read piece: {}", e);
                                        return;
                                    }
                                };

                                // send it back over TURN
                                let reply = TurnPacket {
                                    session_id: session_id.clone(),
                                    target_id: Some(leecher_id.clone()),
                                    body: Some(Body::Piece(TurnPiece { payload: piece, index })),
                                };
                                println!("sending piece: {}", index);
                                if let Err(e) = tx.send(reply).await {
                                    eprintln!("failed to send piece over TURN: {}", e);
                                }
                            }
                        }

                        Some(Err(e)) => {
                            eprintln!("error reading inbound TURN packet: {:?}", e);
                            // you might break here if you want to tear down on error
                            continue;
                        }

                        None => {
                            // client really closed their send‑stream
                            println!("inbound stream closed, exiting Seeder loop");
                            break;
                        }
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
        md.insert("x-role", "leecher".parse().unwrap());


        let mut client_clone = turn_client.clone();
        tokio::spawn(async move {
            match client_clone.send(req).await {
                Ok(_) => eprintln!("TURN send RPC ended normally"),
                Err(e) => eprintln!("TURN send stream ended unexpectedly: {}", e),
            }
        });

        // spawn a task to both receive pieces and requests and process them
        let conn_rx = Arc::clone(&conn_rx);
        //sleep(Duration::from_secs(10)).await;
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
                    if let Some(Message::Request { index, hash, .. }) = request_message {
                        let request_packet = TurnPacket {
                            session_id: session_id.clone(),
                            target_id: Some(leecher_id.clone()),
                            body: Some(Body::Request(TurnPieceRequest {
                                hash: hash.to_vec(),
                                index,
                            })),
                        };
                        println!("requested piece {}", index);
                        // now there *is* a live receiver pulling from `rx`
                        if let Err(e) = tx.send(request_packet).await {
                            eprintln!("failed to queue request: {}", e);
                        }
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