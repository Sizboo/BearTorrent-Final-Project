use crate::connection::connection::*;
use crate::connection::connection::turn_packet::Body;
use crate::turn_server::Turn;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{sync::{mpsc, RwLock, Barrier}, time::{sleep, Duration}};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{async_trait, Request, Response, Status, Streaming, metadata::MetadataMap};

/// role within the turn service session
#[derive(Clone, Copy, Debug)]
pub enum Role {
    Seeder,
    Leecher,
}

/// represents a session between a leecher and seeder in the turn system
pub struct Session {
    seeder:  Option<mpsc::Sender<Result<TurnPacket, Status>>>,
    leecher: Option<mpsc::Sender<Result<TurnPacket, Status>>>,
    barrier: Arc<Barrier>,
}

impl Session {

    /// new (
    ///     barrier: used to synchronize Seeder and Leecher
    /// )
    /// creates a new Session for the turn service, with a barrier used to sync up the seeder and leecher
    pub fn new(barrier: Arc<Barrier>) -> Self {
        Session { seeder: None, leecher: None, barrier }
    }

    /// add_seeder (
    ///     tx: Sender we use to relay to the seeder
    /// )
    /// register a seeder with the Session, error if already has one
    pub fn add_seeder(&mut self, tx: mpsc::Sender<Result<TurnPacket, Status>>) -> Result<(), Status> {
        println!("add_seeder called");
        if self.seeder.is_some() {
            return Err(Status::resource_exhausted("Seeder already registered"));
        }
        self.seeder = Some(tx);
        Ok(())
    }

    /// add_leecher (
    ///     tx: Sender we use to relay to the leecher
    /// )
    /// register a leecher with the Session, error if already has one
    pub fn add_leecher(&mut self, tx: mpsc::Sender<Result<TurnPacket, Status>>) -> Result<(), Status> {
        println!("add_leecher called");
        if self.leecher.is_some() {
            return Err(Status::resource_exhausted("Leecher already registered"));
        }
        self.leecher = Some(tx);
        Ok(())
    }

    /// forward (
    ///     pkt: TurnPacket we are relaying
    /// )
    /// this function checks a packet to see whether it is a Request or a Piece and sends it via the
    /// correct channel. just a simple way to determine who to send the TurnPacket to in a Session
    pub async fn forward(&self, pkt: TurnPacket) {
        println!("Forwarding TurnPacket: {:?}", pkt);
        match &pkt.body {
            Some(Body::Request(_)) => {
                if let Some(tx) = &self.seeder {
                    let _ = tx.send(Ok(pkt.clone())).await;
                }
            }
            Some(Body::Piece(_)) => {
                if let Some(tx) = &self.leecher {
                    let _ = tx.send(Ok(pkt.clone())).await;
                }
            }
            None => {}
        }
    }
}



#[derive(Default)]
pub struct TurnService {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl TurnService {

    /// register_for_session (
    ///     session_id: session_id we are registering to
    ///     role: role for session (Seeder or Leecher)
    /// )
    /// registers a client for the turn service as either a seeder or a leecher
    async fn register_for_session(
        &self,
        session_id: String,
        role: Role,
    ) -> Result<ReceiverStream<Result<TurnPacket, Status>>, Status> {

        // create the channels we will use to relay packets
        let (tx, rx) = mpsc::channel::<Result<TurnPacket, Status>>(128);

        // acquire a write lock for sessions
        let mut all = self.sessions.write().await;

        let session = all.entry(session_id.clone())
            .or_insert_with(|| {
                // create a Barrier for the seeder and leecher
                let barrier = Arc::new(Barrier::new(2));
                Session::new(barrier)
            });

        // fill the right slot
        match role {
            Role::Seeder => session.add_seeder(tx)?,
            Role::Leecher => session.add_leecher(tx)?,
        }

        // wait for both seeder and leecher to get here, then move on
        let barrier = session.barrier.clone();
        drop(all);
        barrier.wait().await;

        Ok(ReceiverStream::new(rx))
    }
}

#[async_trait]
impl Turn for TurnService {
    type registerStream = ReceiverStream<Result<TurnPacket, Status>>;

    /// register (
    ///     req: RegisterRequest we are processing in order to add to the turn service
    /// )
    /// client passes a RegisterRequest and gets registered for turn
    async fn register(
        &self,
        req: Request<RegisterRequest>,
    ) -> Result<Response<ReceiverStream<Result<TurnPacket, Status>>>, Status> {
        let req = req.into_inner();

        // gets role of requester and registers within the turn system
        let role = if req.is_seeder { Role::Seeder } else { Role::Leecher };
        let stream = self.register_for_session(req.session_id, role).await?;
        Ok(Response::new(stream))
    }

    /// send (
    ///     req: TurnPacket stream we use to relay with
    /// )
    /// initiates the turn relay across a session
    async fn send(
        &self,
        req: Request<tonic::Streaming<TurnPacket>>,
    ) -> Result<Response<()>, Status> {
        // unpack metadata
        let metadata = req.metadata();
        let session_id = extract_header(metadata, "x-session-id")?;
        let role = if extract_header(metadata, "x-role")? == "seeder" {
            Role::Seeder
        } else {
            Role::Leecher
        };

        // the main loop we use to relay data via TURN
        let mut inbound = req.into_inner();
        loop {
            match inbound.next().await {
                Some(Ok(pkt)) => {
                    // forward the packet to the correct seeder/leecher
                    if let Some(session) = self.sessions.read().await.get(&session_id) {
                        session.forward(pkt).await;
                    }
                }
                Some(Err(e)) => {
                    return Err(Status::internal(format!(
                        "error reading inbound TURN packet: {:?}",
                        e
                    )));
                }
                None => {
                    break;
                }
            }
        }

        Ok(Response::new(()))
    }

}

/// extract_header (
///     md: the metadata of the stream we are extracting from
///     key: key within the metadata we are extracting
/// )
/// helper function to extract metadata from a stream
fn extract_header(md: &MetadataMap, key: &str) -> Result<String, Status> {
    md.get(key)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned())
        .ok_or_else(|| Status::invalid_argument(format!("missing metadata `{}`", key)))
}