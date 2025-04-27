use crate::connection::{turn_server::Turn, ClientId, TurnPacket};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{async_trait, Request, Response, Status, Streaming};

#[derive(Debug, Default)]
pub struct TurnService {
    sessions: Arc<RwLock<HashMap<ClientId, mpsc::Sender<Result<TurnPacket, Status>>>>>,
}

#[async_trait]
impl Turn for TurnService {
    type RelayStream = ReceiverStream<Result<TurnPacket, Status>>;

    async fn relay
        (&self,
        request: Request<Streaming<TurnPacket>>
    ) -> Result<Response<Self::RelayStream>, Status> {
        let mut inbound = request.into_inner();

        let first = inbound
            .message()
            .await?
            .ok_or_else(|| Status::invalid_argument("expected initial TurnPacket"))?;
        let client_id = first.client_id.clone();

        let (tx, rx) = mpsc::channel::<Result<TurnPacket, Status>>(128);

        self.sessions.write().await.insert(client_id.clone().unwrap(), tx.clone());

        let sessions = Arc::clone(&self.sessions);
        tokio::spawn(async move {
            // Now stream the rest:
            while let Some(Ok(packet)) = inbound.next().await {
                if let Some(peer_tx) = sessions.read().await.get(&packet.target_id.clone().unwrap()) {
                    // Only send to the one intended recipient
                    let _ = peer_tx.send(Ok(packet.clone())).await;
                }
            }

            // Clean up on client disconnect
            sessions.write().await.remove(&client_id.unwrap());
        });

        // Wrap our receiver in a Stream and send it back
        let out_stream = ReceiverStream::new(rx);
        Ok(Response::new(out_stream))
    }
}