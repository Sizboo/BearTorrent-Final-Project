use crate::connection::connection::*;
use crate::connection::connection::turn_packet::Body;
use crate::turn_server::Turn;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{sync::{mpsc, RwLock, Barrier}, time::{sleep, Duration}};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{async_trait, Request, Response, Status, Streaming, metadata::MetadataMap};

/// role within the turn service
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
    pub fn new(barrier: Arc<Barrier>) -> Self {
        Session { seeder: None, leecher: None, barrier }
    }

    /// register the seeder, error if already has one
    pub fn add_seeder(&mut self, tx: mpsc::Sender<Result<TurnPacket, Status>>) -> Result<(), Status> {
        println!("add_seeder called");
        if self.seeder.is_some() {
            return Err(Status::resource_exhausted("Seeder already registered"));
        }
        self.seeder = Some(tx);
        Ok(())
    }

    /// register the leecher, error if already has one
    pub fn add_leecher(&mut self, tx: mpsc::Sender<Result<TurnPacket, Status>>) -> Result<(), Status> {
        println!("add_leecher called");
        if self.leecher.is_some() {
            return Err(Status::resource_exhausted("Leecher already registered"));
        }
        self.leecher = Some(tx);
        Ok(())
    }

    /// forwards packet between the seeder and leecher based on if request or piece

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

    /// disconnect logic
    pub fn remove(&mut self, role: Role) {
        match role {
            Role::Seeder => self.seeder = None,
            Role::Leecher => self.leecher = None,
        }
    }

    /// drop logic
    pub fn is_empty(&self) -> bool {
        self.seeder.is_none() && self.leecher.is_none()
    }
}



#[derive(Default)]
pub struct TurnService {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl TurnService {

    /// registers a client for the turn service as either a seeder or a leecher
    async fn register_for_session(
        &self,
        session_id: String,
        role: Role,
    ) -> Result<ReceiverStream<Result<TurnPacket, Status>>, Status> {
        println!("Registering {:?} for session: {:?}", role, &session_id);
        let (tx, rx) = mpsc::channel::<Result<TurnPacket, Status>>(128);

        let mut all = self.sessions.write().await;

        let session = all.entry(session_id.clone())
            .or_insert_with(|| {
                println!("Creating new session for {:?}", session_id);
                // create a Barrier for exactly two arrivals
                let barrier = Arc::new(Barrier::new(2));
                Session::new(barrier)
            });

        // fill the right slot
        match role {
            Role::Seeder => session.add_seeder(tx)?,
            Role::Leecher => session.add_leecher(tx)?,
        }
        let barrier = session.barrier.clone();
        drop(all);

        barrier.wait().await;
        println!("Both peers registered for session {}", session_id);

        Ok(ReceiverStream::new(rx))
    }
}

#[async_trait]
impl Turn for TurnService {
    type registerStream = ReceiverStream<Result<TurnPacket, Status>>;

    /// client passes a RegisterRequest and gets registered for turn
    async fn register(
        &self,
        req: Request<RegisterRequest>,
    ) -> Result<Response<ReceiverStream<Result<TurnPacket, Status>>>, Status> {
        let req = req.into_inner();
        let role = if req.is_seeder { Role::Seeder } else { Role::Leecher };
        let stream = self.register_for_session(req.session_id, role).await?;
        Ok(Response::new(stream))
    }

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

        let mut inbound = req.into_inner();
        loop {
            match inbound.next().await {
                Some(Ok(pkt)) => {
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

fn extract_header(md: &MetadataMap, key: &str) -> Result<String, Status> {
    md.get(key)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned())
        .ok_or_else(|| Status::invalid_argument(format!("missing metadata `{}`", key)))
}