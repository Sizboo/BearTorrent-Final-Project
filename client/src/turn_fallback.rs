use crate::connection::connection::{turn_client::TurnClient, PeerId, RegisterRequest, TurnPacket,
                                    turn_packet::Body, TurnPiece, TurnPieceRequest, InfoHash};
use crate::message::Message;
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::Status;
use std::sync::Arc;
use tokio::{sync::{Mutex, RwLock}};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use crate::file_handler::{read_piece_from_file };

pub struct TurnFallback {
}

impl TurnFallback {

    // start_seeding(
    //     turn_client: a client's way to access the turn service on the server
    //     seeder_id: their peer_id
    //     leecher_id: the peer_id of the leecher they are registering for the TURN service with
    //     file_map: the map used to identify files
    // )
    //
    // function to start seeding via our TURN service on the server
    pub async fn start_seeding(
        mut turn_client: TurnClient<tonic::transport::Channel>,
        seeder_id: PeerId,
        leecher_id: PeerId,
        file_map: Arc<RwLock<HashMap<[u8; 20], InfoHash>>>,
    ) -> Result<(), Status> {
        let session_id = make_session_id(&seeder_id, &leecher_id);

        // register this client as a seeder for the turn service and gets the mpsc::Receiver back
        // to receive data from the TURN service
        let inbound = turn_client
            .register(RegisterRequest {
                session_id: session_id.clone(),
                is_seeder: true,
            })
            .await?
            .into_inner();

        // create our channels and wrap the rx in a ReceiverStream to send to the turn service
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);
        let outbound = ReceiverStream::new(rx);

        // build the request and insert metadata
        let mut req = tonic::Request::new(outbound);
        println!("made it past sending req");
        let md = req.metadata_mut();
        md.insert("x-session-id", session_id.parse().unwrap());
        md.insert("x-role", "seeder".parse().unwrap());

        // signal for the turn service to start relaying our data
        let mut client_clone = turn_client.clone();
        tokio::spawn(async move {
            match client_clone.send(req).await {
                Ok(_) => eprintln!("TURN send RPC ended normally"),
                Err(e) => eprintln!("TURN send stream ended unexpectedly: {}", e),
            }
        });

        // this is the main seeding loop we will use to read requests we receive from the leecher (via turn),
        // grab the corresponding piece, and send it back to the leecher (via turn)
        tokio::spawn({
            let mut inbound = inbound;
            let file_map = file_map.clone();
            let session_id = session_id.clone();
            let tx = tx.clone();

            async move {
                loop {
                    match inbound.next().await {
                        Some(Ok(pkt)) => {
                            if let Some(Body::Request(req)) = pkt.body {
                                let index: u32 = req.index;

                                // turn bytes into a fixed [u8;20] hash that we need
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

                                // grab the piece from the file
                                let piece = match read_piece_from_file(info_hash, index) {
                                    Ok(vec) => vec,
                                    Err(e) => {
                                        eprintln!("failed to read piece: {}", e);
                                        return;
                                    }
                                };

                                // load the packet and send it via turn
                                let reply = TurnPacket {
                                    session_id: session_id.clone(),
                                    body: Some(Body::Piece(TurnPiece { payload: piece, index })),
                                };
                                if let Err(e) = tx.send(reply).await {
                                    eprintln!("failed to send piece over TURN: {}", e);
                                }
                            }
                        }

                        Some(Err(e)) => {
                            eprintln!("error reading inbound TURN packet: {:?}", e);
                            continue;
                        }

                        None => {
                            println!("inbound stream closed, exiting Seeder loop");
                            break;
                        }
                    }
                }
            }
        });

        Ok(())
    }


    /// start_seeding(
    ///     turn_client: a client's way to access the turn service on the server
    ///     leecher_id: their peer_id
    ///     seeder_id: the peer_id of the seeder they are registering for the TURN service with
    ///     conn_tx: the Sender used to send pieces to our file assembly system
    ///     conn_rx: the Receiver used to get Requests from
    /// )
    /// function to start leeching via TURN
    pub async fn start_leeching(
        mut turn_client: TurnClient<tonic::transport::Channel>,
        leecher_id: PeerId,
        seeder_id: PeerId,
        conn_tx: mpsc::Sender<Message>,
        conn_rx: Arc<Mutex<mpsc::Receiver<Message>>>,
    ) -> Result<(), Status> {
        let session_id = make_session_id(&seeder_id, &leecher_id);

        // register for turn as the leecher
        let mut inbound = turn_client
            .register(RegisterRequest {
                session_id: session_id.clone(),
                is_seeder: false,
            })
            .await?
            .into_inner();

        // create a channel to send requests through and wrap rx in a ReceiverStream
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

        // signal for the turn service to relay our data
        let mut client_clone = turn_client.clone();
        tokio::spawn(async move {
            match client_clone.send(req).await {
                Ok(_) => eprintln!("TURN send RPC ended normally"),
                Err(e) => eprintln!("TURN send stream ended unexpectedly: {}", e),
            }
        });

        // spawn a task to both receive pieces and requests and process them
        let conn_rx = Arc::clone(&conn_rx);
        println!("made it to Leecher loop");
        loop {
            tokio::select! {
                // if we receive a piece from the seeder (via turn), process it and sent it off to
                // our file assembly system
                turn_packet = inbound.next() => {
                    match turn_packet {
                        Some(Ok(pkt)) => {
                            if let Some(Body::Piece(tp)) = pkt.body {

                                let piece_msg = Message::Piece {
                                    index: tp.index,
                                    piece: tp.payload,
                                };

                                let res = conn_tx.send(piece_msg).await;
                                if res.is_err() {
                                    eprintln!("failed to send piece");
                                }
                            }
                        }
                        _ => continue,
                    }
                }

                // if we receive a request from our piece-requesting system, process it and send it via turn
                request_message = async {
                    let mut rx = conn_rx.lock().await;
                    rx.recv().await
                } => {
                    if let Some(Message::Request { index, hash, .. }) = request_message {
                        let request_packet = TurnPacket {
                            session_id: session_id.clone(),
                            body: Some(Body::Request(TurnPieceRequest {
                                hash: hash.to_vec(),
                                index,
                            })),
                        };

                        if let Err(e) = tx.send(request_packet).await {
                            eprintln!("failed to queue request: {}", e);
                        }
                    }
                }
            }
        }

    }
}
/// peer_to_string(
///     peer: PeerId we are converting to a string
/// )
/// helper function for creating a string from a peerid (for use in make_session_id)
fn peer_to_string(peer: &PeerId) -> String {
    let public_ip  = Ipv4Addr::from(peer.ipaddr);
    let private_ip = Ipv4Addr::from(peer.priv_ipaddr);
    format!("{}:{}-{}:{}", public_ip, peer.port, private_ip, peer.priv_port)
}

/// make_session_id (
///     a: PeerId a that we are converting to session_id
///     b: PeerId a that we are converting to session_id
/// )
/// fucntion for creating a session_id as a string for properly identifying the turn session that
/// the seeder/leecher is a part of
fn make_session_id(a: &PeerId, b: &PeerId) -> String {
    let key_a = (a.ipaddr, a.port, a.priv_ipaddr, a.priv_port);
    let key_b = (b.ipaddr, b.port, b.priv_ipaddr, b.priv_port);
    let (first, second) = if key_a <= key_b { (a, b) } else { (b, a) };
    format!("{}|{}", peer_to_string(first), peer_to_string(second))
}
