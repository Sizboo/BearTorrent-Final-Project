mod turn;
mod connection;

use std::{env, sync::Arc};
use std::time::Duration;
use dashmap::DashMap;
use tonic::{transport::Server, Code, Request, Response, Status};
use connection::connection::*;
use crate::connector_server::{Connector, ConnectorServer};
use crate::turn_server::TurnServer;
use tokio::sync::{mpsc, watch};
use tokio::time::{sleep};
use uuid::Uuid;
use crate::turn::TurnService;


#[derive(Debug, Default)]
pub struct ConnectionService {
    client_registry: Arc<DashMap<ClientId, Option<PeerId>>>,
    file_tracker: Arc<DashMap<FileHash, InfoHash>>,
    seeder_list: Arc<DashMap<FileHash, Vec<ClientId>>>,
    seed_notifier: Arc<DashMap<PeerId, mpsc::Sender<PeerId>>>,
    cert_sender: Arc<DashMap<PeerId, mpsc::Sender<Cert>>>,
    init_hole_punch: Arc<DashMap<PeerId, watch::Sender<bool>>>,
}

impl ConnectionService {
}

#[tonic::async_trait]
impl Connector for ConnectionService {

    /// this function is used for a client to request a file from the server
    /// it returns the list of peers that have the file, from the tracker map
    async fn get_file_peer_list(
        &self,
        request: Request<FileHash>,
    ) -> Result<Response<PeerList>, Status> {
        let info_hash = request.into_inner();

        if let Some(clients) = self.seeder_list.get(&info_hash) {

            let client_map = self.client_registry.clone();
            // todo maybe resolve unwarp here
            let peer_list = clients.iter().filter_map( |s| client_map.get(&s)?.to_owned()).collect();

            Ok(Response::new(PeerList { list: peer_list }))
        } else {
            // just returns an empty list
            Ok(Response::new(PeerList { list: vec![] }))
        }
    }

    async fn send_file_request(
        &self,
        request: Request<ConnectionIds>
    ) -> Result<Response<()>, Status> {
        let r = request.into_inner();
        //this is the connection id retrieved from get_file_peer_list() of the peer seeding
        let seeder_peer_id = r.connection_peer.ok_or(Status::invalid_argument("missing peer id"))?;

        //this is your own client_id so they can find your connection id
        let self_id = r.self_id.ok_or(Status::invalid_argument("missing self"))?;

        //loop through 3 times after delay.
        //this just gives multiple attempts in case of network delay or seeding at bad times
        for i in 0 ..5 {
            if let Some(entry) = self.seed_notifier.remove(&seeder_peer_id) {
                let tx = entry.1;
                tx.send(self_id).await.map_err(|_| Status::internal("Failed to send self_id"))?;
                break;
            } else {
                sleep(Duration::from_millis(250)).await;
                if i == 4 {
                    //todo check for this error and initiate a new connection if it occurs
                    return Err(Status::not_found("missing seeding peer"));
                }
                continue;
            }

        }

        Ok(Response::new(()))
    }

    ///this function is used by clients willing to share data to get peers who request data.
    /// clients should listen to this service at all times they are willing to send.
    async fn seed(
        &self,
        request: Request<PeerId>
    ) -> Result<Response<PeerId>, Status> {
        let self_peer_id = request.into_inner();

        let (peer_id_tx, mut peer_id_rx) = mpsc::channel::<PeerId>(1);

        self.seed_notifier.insert(self_peer_id, peer_id_tx);

        //get id of client requesting file
        let peer_id= peer_id_rx.recv().await
            .ok_or(Status::new(Code::Internal, "Failed to receive client id"))?;

        Ok(Response::new(peer_id))
    }
    
    /// await_trigger_hole_punch() should be used by the sending peer (ie the peer with the data)
    /// and awaited to initiate its hole punch sequence. 
    /// When this method returns, it indicates the requesting client is beginning its hole punch sequence. 
    async fn await_hole_punch_trigger(
        &self,
        request: Request<PeerId>,
    ) -> Result<Response<()>, Status> {
        let self_id = request.into_inner();

        let (watch_tx, mut watch_rx) = watch::channel(false);

        self.init_hole_punch.insert(self_id, watch_tx);

        watch_rx.changed().await.map_err(|e| Status::new(Code::Internal, e.to_string()))?;

        Ok(Response::new(()))
        
    }
    
    /// init_hole_punch() is used to notify a seeding peer that they should begin the udp hole punching procedure.
    /// This function should be called right before the calling peer initiates their own hole punching procedure
    /// as UDP hole punching is time-sensitive.
    async fn init_punch(
        &self,
        request: Request<PeerId>
    ) -> Result<Response<()>, Status> {
       
        let peer_id = request.into_inner();
  
        for i in 0..5 {
            match self.init_hole_punch.get(&peer_id) {
                Some(notify_handle) => {
                    println!("Hole Punch notifier received by Leecher");
                    notify_handle.send(true).map_err(|e| Status::internal(e.to_string()))?;
                    break;
                },
                None => {
                    Err(Status::internal("no seeding peer"))?;
                    sleep(Duration::from_millis(250)).await;
                    if i == 4 {
                        return Err(Status::invalid_argument("invalid peer id"))?;
                    }
                }
            }
        }
        
        Ok(Response::new(()))
    }

    /// this function is used to advertise a client owns a file that can be shared 
    async fn advertise(
        &self,
        request: Request<FileMessage>,
    ) -> Result<Response<ClientId>, Status> {
        let r = request.into_inner();

        let file_hash = r.hash.ok_or(Status::invalid_argument("missing file hash"))?;
        let info_hash = r.info_hash.ok_or(Status::invalid_argument("missing info hash"))?;


        let client_id = match r.id {
            Some(id) => id,
            None => return Err(Status::invalid_argument("Client missing")),
        };

        self.file_tracker.insert(file_hash.clone(), info_hash);

        self.seeder_list
            .entry(file_hash)
            .or_insert_with(Vec::new)
            .push(client_id.clone());

        Ok( Response::new(client_id) )
    }

    async fn register_client(
        &self,
        request: Request<ClientRegistry>,
    ) -> Result<Response<ClientId>, Status> {
        
        let mut uid;
        
        loop {
            let uuid = Uuid::new_v4();
            uid = ClientId{ uid: uuid.to_string()};
            
            if !self.client_registry.contains_key(&uid) {
                break;
            }

        }
        
        if uid.uid.is_empty() {
            return Err(Status::internal("failed to generate uid"))?
        }
        
        self.client_registry.insert(uid.clone(), request.into_inner().peer_id);

        Ok(Response::new(uid ))
    }

    
    /// update_registered_peer_id() is used to update the peer id of a client that has been registered
    /// this is used when a client changes their ip address
    async fn update_registered_peer_id(
        &self,
        request: Request<FullId>
    ) -> Result<Response<ClientId>, Status> {
    
        let r = request.into_inner();
        let self_id = r.self_id.ok_or(Status::invalid_argument("self id not provided"))?;
        let peer_id = r.peer_id.ok_or(Status::invalid_argument("peer id not provided"))?;
        
        self.client_registry.insert(self_id.clone(), Some(peer_id));

        Ok(Response::new(self_id))
    
    }


    /// get_cer() is used by the client end of the quic connection to get the server's self-signed certificate
    /// it should be called and waited upon once init_cert_sender() has been called
    /// get_cert() SHOULD NOT be called unless init_cert_sender() has completed
    async fn get_cert(
        &self,
        request: Request<PeerId>
    ) -> Result<Response<Cert>, Status> {
        let self_addr = request.into_inner();

        let (cert_tx, mut cert_rx) = mpsc::channel::<Cert>(1);

        self.cert_sender.insert(self_addr, cert_tx);
        let cert = cert_rx.recv().await.ok_or(Status::internal("failed to receive cert from server"))?;

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

        for i in 0..5 {
            match self.cert_sender.remove(&peer_id) {
                Some(entry) => {
                    let cert_tx = entry.1;
                    cert_tx.send(r.cert.clone().unwrap()).await
                    .map_err(|e| Status::internal(e.to_string()))?;
                    break;
                }
                None => {
                    sleep(Duration::from_millis(250)).await;
                    if i == 4 {
                        return Err(Status::invalid_argument("Cert sender not registered"))?;
                    }
                    continue
                }
            }
        }
        
        Ok(Response::new(()))
    }

    async fn get_client_id(
        &self,
        request: Request<PeerId>,
    ) -> Result<Response<ClientId>, Status> {
        let peer = request.into_inner();

        let registry = self.client_registry.clone();

        let client_id = registry
            .iter()
            .find_map(|entry| {
                let (client_id, saved_peer) = entry.pair();
                if saved_peer.unwrap() == peer {
                    Some(client_id.clone())
                } else {
                    None
                }
            })
            .ok_or_else(|| Status::not_found("No client registered for that peer"))?;

        Ok(Response::new(client_id))
    }
    
    async fn get_all_files(
        &self,
        _request: Request<()>
    ) -> Result<Response<FileList>, Status> {
        
        let info_hashes = self.file_tracker
            .iter()
            // get all the values (1) in the map
            .map(|x| x.value().clone()).collect::<Vec<_>>();
        
        Ok(Response::new(
            FileList {
                info_hashes,
            }
        ))
        
    }

    async fn delete_file(
        &self,
        request: Request<FileDelete>
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        let self_id = req.id.ok_or(Status::invalid_argument("missing self id"))?;
        let file_hash = req.hash.ok_or(Status::invalid_argument("missing file hash"))?;
        
        if let Some(mut entry) = self.seeder_list.get_mut(&file_hash) {
            let seeders = entry.value_mut();
            seeders.retain(|client_id| *client_id != self_id );
           
            
            //If this was the last seeder who had this file, we want ot remove it from both
            //file_tracker and seeder_list. So that it is no longer advertised to peers.
            if seeders.is_empty() {
                drop(entry);
                self.seeder_list.remove(&file_hash);
                self.file_tracker.remove(&file_hash);
            }
        }

       Ok(Response::new(()))
    }
    
    async fn delist_client(
        &self,
        request: Request<ClientId>,
    ) -> Result<Response<()>, Status> {
        let client_id = request.into_inner();
        
        if let Some(client_registry_entry) = self.client_registry.remove(&client_id) {
            
            //if peer_id is found anywhere remove it
            if let Some(peer_id) = client_registry_entry.1 {
                if self.seed_notifier.contains_key(&peer_id) {
                    self.seed_notifier.remove(&peer_id);    
                }
                if self.cert_sender.contains_key(&peer_id) {
                    self.cert_sender.remove(&peer_id);
                }
                if self.init_hole_punch.contains_key(&peer_id) {
                    self.init_hole_punch.remove(&peer_id);
                }
            }
            
            //remove from seeding list
            self.seeder_list.iter_mut()
                .for_each(|mut entry| {
                    entry.value_mut().retain(|id| *id != client_id);
                    if entry.value().is_empty() {
                        self.file_tracker.remove(entry.key());
                    }
                });
        }

        Ok(Response::new(()))
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