use std::env;
use std::collections::HashMap;
use tonic::{transport::Server, Request, Response, Status};
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
    pub tracker: Arc<Mutex<HashMap<u32, Vec<Seeder>>>>,
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

        let mut tracker = self.tracker.lock().await;

        // notify all seeders of the file
        if let Some(seeders) = tracker.get_mut(&r.info_hash) {
            for seeder in seeders.iter_mut() {
                let temp = seeder.notify.send(requester.clone()).await;
            }

            // returns a list of all peers that have a file
            let peer_list = seeders.iter().map(|s| s.peer.clone()).collect();
            Ok(Response::new(PeerList { list: peer_list }))
        } else {
            // just returns an empty list
            Ok(Response::new(PeerList { list: vec![] }))
        }
    }


    /// this function is used to get a peer when the seeding process is listening,
    /// the server will send a PeerId to a seeding client for them to start streaming data
    async fn get_peer(
        &self,
        request: Request<PeerId>,
    ) -> Result<Response<PeerId>, Status> {
        let r = request.into_inner();
        
        let ipaddr = r.ipaddr;
        let port = r.port;

        let (tx, mut rx) = mpsc::channel(1);

        let mut tracker = self.tracker.lock().await;
        tracker.entry(r.info_hash).or_default().push(Seeder {
            peer: PeerId {
                ipaddr,
                port,
            },
            notify: tx,
        });
        drop(tracker);

        match rx.recv().await {
            Some(peer) => {
                Ok(Response::new(peer))
            }
            None => Err(Status::internal("dropped")),
        }
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