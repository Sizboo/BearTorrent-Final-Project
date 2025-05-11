mod turn;
mod connection;

use std::{env, collections::HashMap, sync::Arc};
use std::time::Duration;
use tonic::{transport::Server, Code, Request, Response, Status};
use connection::connection::*;
use crate::connector_server::{Connector, ConnectorServer};
use crate::turn_server::TurnServer;
use tokio::sync::{Mutex, mpsc, Notify, watch};
use tokio::sync::RwLock;
use tokio::time::timeout;
use uuid::Uuid;
use crate::turn::TurnService;

#[derive(Debug, Default)]
pub struct ConnectionService {
    client_registry: Arc<RwLock<HashMap<ClientId, Option<PeerId>>>>,
    file_tracker: Arc<Mutex<HashMap<InfoHash, Vec<ClientId>>>>,
    seed_tracker: Arc<RwLock<HashMap<PeerId, mpsc::Receiver<ClientId>>>>,
    request_tracker: Arc<RwLock<HashMap<PeerId, mpsc::Sender<ClientId>>>>,
    cert_sender: Arc<RwLock<HashMap<PeerId, (mpsc::Sender<Cert>, Option<mpsc::Receiver<Cert>>)>>>,
    //todo refactor this to use a send channel
    init_hole_punch: Arc<RwLock<HashMap<PeerId, watch::Sender<bool>>>>,
}

impl ConnectionService {
}

#[tonic::async_trait]
impl Connector for ConnectionService {

    /// this function is used for a client to request a file from the server
    /// it returns the list of peers that have the file, from the tracker map
    async fn get_file_peer_list(
        &self,
        request: Request<InfoHash>,
    ) -> Result<Response<PeerList>, Status> {
        let info_hash = request.into_inner();

        let mut file_tracker = self.file_tracker.lock().await;

        // notify all seeders of the file

        if let Some(clients) = file_tracker.get_mut(&info_hash) {

            let map = self.client_registry.read().await;
            // todo maybe resolve unwarp here
            let peer_list = clients.iter().filter_map( |s| map.get(&s).cloned().unwrap()).collect();

            Ok(Response::new(PeerList { list: peer_list }))
        } else {
            // just returns an empty list
            Ok(Response::new(PeerList { list: vec![] }))
        }
    }

    async fn send_file_request(&self, request: Request<FullId>) -> Result<Response<()>, Status> {
        let r = request.into_inner();
        //this is the connection id retrieved from get_file_peer_list() of the peer seeding
        let seeder_peer_id = r.peer_id.ok_or(Status::invalid_argument("missing peer id"))?;

        //this is your own client_id so they can find your connection id
        let self_id = r.self_id.ok_or(Status::invalid_argument("missing self"))?;

        let tx = self.request_tracker.write().await.remove(&seeder_peer_id)
            .ok_or(Status::new(Code::Internal, "Failed to get peer ID from request_trackker"))?;

        tx.send(self_id).await.map_err(|_| Status::internal("Failed to send self_id"))?;

        Ok(Response::new(()))
    }

    ///this function is used by clients willing to share data to get peers who request data.
    /// clients should listen to this service at all times they are willing to send.
    //todo consider renaming
    async fn seed(&self, request: Request<PeerId>) -> Result<Response<PeerId>, Status> {
        let self_peer_id = request.into_inner();
        let mut recv = self.seed_tracker.write().await.remove(&self_peer_id)
            .ok_or(Status::new(Code::Internal, "No seed tracker"))?;

        //get id of client requesting file
        let peer_client_id = recv.recv().await.ok_or(Status::new(Code::Internal, "Failed to receive client id"))?;

        //find registered peer_id to make connection to
        let peer_id = self.client_registry.read().await.get(&peer_client_id)
            .ok_or(Status::new(Code::Internal, "failed to removed peer id from map"))?
            .ok_or(Status::new(Code::Unavailable, "No peer id returned"))?;

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

        self.init_hole_punch.write().await.insert(self_id, watch_tx);

        watch_rx.changed().await.map_err(|e| Status::new(Code::Internal, e.to_string()))?;
        
        Ok(Response::new(()))
        
    }
    
    /// init_hole_punch() is used to notify a seeding peer that they should begin the udp hole punching procedure.
    /// This function should be called right before the calling peer initiates their own hole punching procedure
    /// as UDP hole punching is time-sensitive.
    async fn init_punch(&self, request: Request<PeerId>) -> Result<Response<()>, Status> {
       
        let peer_id = request.into_inner();
  
        
        match self.init_hole_punch.write().await.remove(&peer_id) {
            Some(notify_handle) => {
                println!("Hole Punch notifier received by Leecher");
                notify_handle.send(true).map_err(|e| Status::internal(e.to_string()))?;
            },
            None => {
                Err(Status::internal("no seeding peer"))?;
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
        // println!("advertising file: {:?}", r.info_hash);

        let client_id = match r.id {
            Some(id) => id,
            None => return Err(Status::invalid_argument("Client missing")),
        };

        let (tx, rx) = mpsc::channel(1);

        
        if let Some(info_hash) = r.info_hash {
            let mut file_tracker = self.file_tracker.lock().await;
            file_tracker.entry(info_hash).or_default().push(client_id.clone());
        } else {
            Err(Status::invalid_argument("info_hash missing"))?
        }
        let self_conn_id = self.client_registry.read().await.get(&client_id)
            .ok_or(Status::internal("could not read from client_registry"))?
            .ok_or(Status::invalid_argument("client_id missing"))?;

        self.seed_tracker.write().await.insert(self_conn_id.clone(), rx);
        self.request_tracker.write().await.insert(self_conn_id, tx);

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
            
            if !self.client_registry.read().await.contains_key(&uid) {
                break;
            }

        }
        
        if uid.uid.is_empty() {
            return Err(Status::internal("failed to generate uid"))?
        }
        
        self.client_registry.write().await.insert(uid.clone(), request.into_inner().peer_id);

        Ok(Response::new(uid ))
    }

    
    /// update_registered_peer_id() is used to update the peer id of a client that has been registered
    /// this is used when a client changes their ip address
    async fn update_registered_peer_id(&self, request: Request<FullId>) -> Result<Response<ClientId>, Status> {
    
        let r = request.into_inner();
        let self_id = r.self_id.ok_or(Status::invalid_argument("self id not provided"))?;
        let peer_id = r.peer_id.ok_or(Status::invalid_argument("peer id not provided"))?;
        
        self.client_registry.write().await.insert(self_id.clone(), Some(peer_id));

        Ok(Response::new(self_id))
    
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
        request: Request<()>
    ) -> Result<Response<FileList>, Status> {
        
        let info_hashes = self.file_tracker.lock().await.keys().map(|x| x.clone()).collect::<Vec<_>>();
        
        Ok(Response::new(
            FileList {
                info_hashes,
            }
        ))
        
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