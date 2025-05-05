mod turn;

use std::{env, collections::HashMap, sync::Arc};
use tonic::{transport::Server, Code, Request, Response, Status};
use connection::{PeerId, PeerList, FileMessage, connector_server::{Connector, ConnectorServer}};
use tokio::sync::{Mutex, mpsc};
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::connection::{Cert, CertMessage, ClientId};
use crate::connection::turn_server::TurnServer;
use crate::turn::TurnService;

pub mod connection {
    tonic::include_proto!("connection");
}

/// storing this in the tracker map so we can contact clients in the seeder process
#[derive(Debug)]
struct Seeder {
    client_id: ClientId,
    notify: mpsc::Sender<ClientId>,
}


#[derive(Debug, Default)]
pub struct ConnectionService {
    client_registry: Arc<RwLock<HashMap<ClientId, PeerId>>>,
    file_tracker: Arc<Mutex<HashMap<u32, Vec<Seeder>>>>,
    send_tracker: Arc<Mutex<HashMap<ClientId, mpsc::Receiver<ClientId>>>>,
    cert_sender: Arc<RwLock<HashMap<PeerId, (mpsc::Sender<Cert>, Option<mpsc::Receiver<Cert>>)>>>,
}

impl ConnectionService {
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
        let requester = r.clone().id.unwrap_or(ClientId { uid: "".to_string() });

        let mut file_tracker = self.file_tracker.lock().await;

        // notify all seeders of the file
        if let Some(seeders) = file_tracker.get_mut(&r.info_hash) {
            //todo implement this so that it selects specific peers (or pieces out file)
            for seeder in seeders.iter_mut() {
                
                let res = seeder.notify.send(requester.clone()).await;
                
                if res.is_err() {
                    return Err(Status::internal("failed to send to seeder"))?
                }
            }

            // returns a list of all peers that have a file
            //todo this needs to send THE seeder/s that are sharing for hole punching, not necessarily everyone with file
            let map = self.client_registry.read().await;
            let peer_list = seeders.iter().filter_map( |s| map.get(&s.client_id).cloned()).collect();
            Ok(Response::new(PeerList { list: peer_list }))
        } else {
            // just returns an empty list
            Ok(Response::new(PeerList { list: vec![] }))
        }
    }

    ///this function is used by clients willing to share data to get peers who request data.
    /// clients should listen to this service at all times they are willing to send.
    //todo consider renaming
    async fn get_peer(&self, request: Request<ClientId>) -> Result<Response<PeerId>, Status> {
        println!("get_peer called");
        let client_id = request.into_inner(); 
        
        //todo if we implement states (offline, seeding) should first update its state on server to sharing
        // any time in offline status it will not be selected


        match self.send_tracker.lock().await.get_mut(&client_id) {
            Some(recv) => {
                let client_id = recv.recv().await
                    .ok_or(Status::new(Code::Internal, "peer id not returned upon signal from server"))?;
                
                let peer_id = self.client_registry.read().await
                    .get(&client_id).cloned().ok_or(Status::internal("failed to get peer id"))?;
                
                Ok(Response::new(peer_id))
            }
            None => Err(Status::internal("dropped")),
        }


    }

    /// this function is used to advertise a client owns a file that can be shared 
    async fn advertise(
        &self,
        request: Request<FileMessage>,
    ) -> Result<Response<ClientId>, Status> {
        let r = request.into_inner();

        println!("advertising file: {:}", r.info_hash);

        let client_id = match r.id {
            Some(id) => id,
            None => return Err(Status::invalid_argument("Client missing")),
        };

        let (tx, rx) = mpsc::channel(1);

        let mut file_tracker = self.file_tracker.lock().await;
        file_tracker.entry(r.info_hash).or_default().push(Seeder {
            client_id: client_id.clone(),
            notify: tx,
        });

        let mut send_tracker = self.send_tracker.lock().await;
        send_tracker.insert(client_id.clone(), rx);

        Ok( Response::new(client_id) )
    }

    async fn register_client(
        &self,
        request: Request<PeerId>,
    ) -> Result<Response<ClientId>, Status> {
        
        let mut uid;
        
        loop {
            let uuid = Uuid::new_v4();
            uid = ClientId{ uid: uuid.to_string()};
            
            if !self.client_registry.read().await.contains_key(&uid) {
                break;
            }

        }
        
        if uid.uid.is_empty() {
            return Err(Status::internal("failed to generate uid"))?
        }
        
        self.client_registry.write().await.insert(uid.clone(), request.into_inner());

        Ok(Response::new(uid ))
    }

    /// init_cert_sender() must be called before a quic connection can be established
    /// this will be used to send the self-signed cert from the "server" to "client"
    ///
    /// Convention:
    ///     Sending Peer = Quic Server
    ///     Receiving Peer = Quic Client
    ///
    /// Therefore, the receiving peer should call init_cert_sender prior to hole punching
    /// to ensure it can receive the server's self-signed certificate
    async fn init_cert_sender(
        &self,
        request: Request<PeerId>
    ) -> Result<Response<()>, Status> {
        let r = request.into_inner();

        //TODO determine type (it will be cert)
        let (tx, rx ) = mpsc::channel::<Cert>(1);
        self.cert_sender.write().await.insert(r, (tx, Some(rx)));

        Ok(Response::new(()))
    }
    /// get_cer() is used by the client end of the quic connection to get the server's self-signed certificate
    /// it should be called and waited upon once init_cert_sender() has been called
    /// get_cert() SHOULD NOT be called unless init_cert_sender() has completed
    async fn get_cert(
        &self,
        request: Request<PeerId>
    ) -> Result<Response<Cert>, Status> {
        let self_addr = request.into_inner();

        let mut cert_map = self.cert_sender.write().await;

        let (_cert_send, recv) = cert_map.get_mut(&self_addr).ok_or(Status::not_found("Cert sender not registered"))?;
        let mut cert_recv = std::mem::take(recv).ok_or_else(|| Status::internal("failed to receive cert from server"))?;
        drop(cert_map);

        let cert = cert_recv.recv().await.ok_or(Status::internal("failed to receive cert from server"))?;

        Ok(Response::new(cert))
    }

    ///Send Cert should be used by the server side of quic connection to communicate self-signed cert to the client
    async fn send_cert(
        &self,
        request: Request<CertMessage>
    ) -> Result<Response<()>, Status> {
        let r = request.into_inner();
        
        let peer_id = match r.peer_id {
            Some(id) => id,
            None => return Err(Status::invalid_argument("peer id not returned upon signal from server"))?
        };
        let cert_map = self.cert_sender.read().await;
        let (cert_tx, _cert_rx) = cert_map.get(&peer_id)
            .ok_or(Status::not_found("Cert sender not registered"))?;
        
        let res = cert_tx.send(r.cert.unwrap()).await;
        
        if res.is_err() {
            return Err(Status::internal("failed to send cert to server"))
        }

        Ok(Response::new(()))
    }

    async fn get_client_id(
        &self,
        request: Request<PeerId>,
    ) -> Result<Response<ClientId>, Status> {
        let peer = request.into_inner();

        let registry = self.client_registry.read().await;

        let client_id = registry
            .iter()
            .find_map(|(client_id, saved_peer)| {
                if saved_peer == &peer {
                    Some(client_id.clone())
                } else {
                    None
                }
            })
            .ok_or_else(|| Status::not_found("No client registered for that peer"))?;

        Ok(Response::new(client_id))
    }
}


        
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let address = format!("0.0.0.0:{}", port).parse()?;
    
    let connection_service = ConnectionService::default();
    let turn_service = TurnService::default();
    
    Server::builder()
        .add_service(ConnectorServer::new(connection_service))
        .add_service(TurnServer::new(turn_service))
        .serve(address)
        .await?;

    Ok(())
}