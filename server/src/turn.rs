use crate::connection::connection::{turn_server::Turn, ClientId, TurnPacket};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{async_trait, Request, Response, Status, Streaming};


use crate::connection::TurnPacket;
use crate::connection::ClientId;
use tokio::sync::mpsc;
use tonic::Status;

/// role within the turn service
#[derive(Clone, Copy)]
pub enum Role {
    Seeder,
    Leecher,
}

/// represents a session between a leecher and seeder in the turn system
pub struct Session {
    seeder:  Option<(ClientId, mpsc::Sender<TurnPacket>)>,
    leecher: Option<(ClientId, mpsc::Sender<TurnPacket>)>,
}

impl Session {
    pub fn new() -> Self {
        Session { seeder: None, leecher: None }
    }

    /// register the seeder, error if already has one
    pub fn add_seeder(&mut self, id: ClientId, tx: mpsc::Sender<TurnPacket>) -> Result<(), Status> {
        if self.seeder.is_some() {
            return Err(Status::resource_exhausted("Seeder already registered"));
        }
        self.seeder = Some((id, tx));
        Ok(())
    }

    /// register the leecher, error if already has one
    pub fn add_leecher(&mut self, id: ClientId, tx: mpsc::Sender<TurnPacket>) -> Result<(), Status> {
        if self.leecher.is_some() {
            return Err(Status::resource_exhausted("Leecher already registered"));
        }
        self.leecher = Some((id, tx));
        Ok(())
    }

    /// forwards packet between the seeder and leecher based on if request or piece
    pub async fn forward(&self, pkt: TurnPacket) {
        match pkt.body {
            Some(Body::Request(req)) => {
                if let Some((_, tx)) = &self.seeder {
                    tx.send(pkt).await;
                }
            }
            Some(Body::Piece(tp)) => {
                if let Some((_, tx)) = &self.leecher {
                    tx.send(pkt).await;
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



#[derive(Debug, Default)]
pub struct TurnService {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl TurnService {

    /// registers a client for the turn service as either a seeder or a leecher
    async fn register_for_session(
        &self,
        session_id: String,
        client_id: ClientId,
        role: Role,
    ) -> Result<ReceiverStream<TurnPacket>, Status> {
        let (tx, rx) = mpsc::channel::<TurnPacket>(128);

        let mut all = self.sessions.write().await;
        let session = all.entry(session_id.clone())
            .or_insert_with(Session::new);

        // fill the right slot
        match role {
            Role::Seeder => session.add_seeder(client_id, tx)?,
            Role::Leecher => session.add_leecher(client_id, tx)?,
        }

        Ok(ReceiverStream::new(rx))
    }

    /// starts the turn relay over a session
    fn start_relay(
        &self,
        mut inbound: tonic::Streaming<TurnPacket>,
        session_id: String,
        client_id: ClientId,
        role: Role,
    ) {
        let sessions = Arc::clone(&self.sessions);

        tokio::spawn(async move {
            while let Some(Ok(pkt)) = inbound.next().await {
                // look up the session and forward by packet type
                if let Some(session) = sessions.read().await.get(&session_id) {
                    session.forward(pkt).await;
                }
            }

            // on disconnect, clean up
            let mut all = sessions.write().await;
            if let Some(session) = all.get_mut(&session_id) {
                session.remove(role);
                if session.is_empty() {
                    all.remove(&session_id);
                }
            }
        });
    }
}

#[async_trait]
impl Turn for TurnService {

    /// client passes a RegisterRequest and gets registered for turn
    async fn register(
        &self,
        req: Request<RegisterRequest>,
    ) -> Result<Response<ReceiverStream<TurnPacket>>, Status> {
        let req = req.into_inner();
        let role = if req.is_seeder { Role::Seeder } else { Role::Leecher };
        let stream = self.register_for_session(req.session_id, req.client_id.unwrap(), role).await?;
        Ok(Response::new(stream))
    }

    /// initiates the turn relay across a session
    async fn send(
        &self,
        req: Request<tonic::Streaming<TurnPacket>>,
    ) -> Result<Response<Empty>, Status> {
        // unpack the metadata attached to the stream
        let metadata = req.metadata();
        let session_id = extract_header(metadata, "x-session-id")?;
        let client_id = ClientId { id: extract_header(metadata, "x-client-id")? };
        let role = if extract_header(metadata, "x-role")? == "seeder" {
            Role::Seeder
        } else {
            Role::Leecher
        };

        self.start_relay(req.into_inner(), session_id, client_id, role);
        Ok(Response::new(Empty {}))
    }
}