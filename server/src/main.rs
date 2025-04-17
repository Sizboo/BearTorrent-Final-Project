use std::env;
use std::collections::HashMap;
use tonic::{transport::Server, Code, Request, Response, Status};
use connection::{PeerId, PeerList, FileMessage, connector_server::{Connector, ConnectorServer}};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
pub mod connection {
    tonic::include_proto!("connection");
}

/// storing this in the tracker map so we can contact clients in the seeder process
#[derive(Debug)]
struct Seeder {
    peer: PeerId,
    notify: mpsc::Sender<PeerId>,
}


#[derive(Debug, Default)]
pub struct ConnectionService {
    file_tracker: Arc<Mutex<HashMap<u32, Vec<Seeder>>>>,
    send_tracker: Arc<Mutex<HashMap<PeerId, mpsc::Receiver<PeerId>>>>,
}

#[tonic::async_trait]
impl Connector for ConnectionService {

    /// this function is used for a client to request a file from the server
    /// it returns the list of peers that have the file, from the tracker map
    async fn send_file_request(
        &self,
        request: Request<FileMessage>,
    ) -> Result<Response<PeerList>, Status> {
        let r = request.into_inner();
        let requester = r.clone().id.unwrap_or(PeerId { ipaddr: 0, port: 0 });

        let mut tracker = self.file_tracker.lock().await;

        // notify all seeders of the file
        if let Some(seeders) = tracker.get_mut(&r.info_hash) {
            for seeder in seeders.iter_mut() {
                seeder.notify.send(requester.clone()).await;
            }

            // returns a list of all peers that have a file
            let peer_list = seeders.iter().map(|s| s.peer.clone()).collect();
            Ok(Response::new(PeerList { list: peer_list }))
        } else {
            // just returns an empty list
            Ok(Response::new(PeerList { list: vec![] }))
        }
    }

    async fn get_peer(&self, request: Request<PeerId>) -> Result<Response<PeerId>, Status> {
        let peer_id = request.into_inner();


        match self.send_tracker.lock().await.get(&peer_id) {
            Some(recv) => {
                let peer_id = recv.recv().await?;
                Ok
            }
            None => Err(Status::internal("dropped")),
        }


    }

    /// this function is used to get a peer when the seeding process is listening,
    /// the server will send a PeerId to a seeding client for them to start streaming data
    async fn advertise(
        &self,
        request: Request<FileMessage>,
    ) -> Result<Response<PeerId>, Status> {
        let r = request.into_inner();

        let peer_id = match r.id {
            Some(id) => id,
            None => return Err(Status::invalid_argument("PeerId missing")),
        };

        let (tx, mut rx) = mpsc::channel(1);

        let mut file_tracker = self.file_tracker.lock().await;
        file_tracker.entry(r.info_hash).or_default().push(Seeder {
            peer: peer_id.clone(),
            notify: tx,
        });

        let mut send_tracker = self.send_tracker.lock().await;
        send_tracker.insert(peer_id, rx);

        Ok( Response::new(peer_id) )
    }

}



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let address = format!("0.0.0.0:{}", port).parse()?;
    let connection_service = ConnectionService::default();

    Server::builder()
        .add_service(ConnectorServer::new(connection_service))
        .serve(address)
        .await?;

    Ok(())
}