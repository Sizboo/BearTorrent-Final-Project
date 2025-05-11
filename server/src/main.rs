mod turn;
mod connection;

use std::{env, collections::HashMap, sync::Arc};
use std::time::Duration;
use tonic::{transport::Server, Code, Request, Response, Status};
use connection::connection::*;
use crate::connector_server::{Connector, ConnectorServer};
use crate::turn_server::TurnServer;
use tokio::sync::{Mutex, mpsc, Notify, watch, oneshot};
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};
use uuid::Uuid;
use crate::turn::TurnService;


#[derive(Debug, Default)]
pub struct ConnectionService {
    client_registry: Arc<RwLock<HashMap<ClientId, Option<PeerId>>>>,
    file_tracker: Arc<RwLock<HashMap<FileHash, InfoHash>>>, 
    seeder_list: Arc<RwLock<HashMap<FileHash, Vec<ClientId>>>>,
    seed_notifier: Arc<RwLock<HashMap<PeerId, mpsc::Sender<PeerId>>>>,
    cert_sender: Arc<RwLock<HashMap<PeerId, (mpsc::Sender<Cert>, Option<mpsc::Receiver<Cert>>)>>>,
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
        request: Request<FileHash>,
    ) -> Result<Response<PeerList>, Status> {
        let info_hash = request.into_inner();

        let seeder_list = self.seeder_list.read().await;

        if let Some(clients) = seeder_list.get(&info_hash) {

            let map = self.client_registry.read().await;
            // todo maybe resolve unwarp here
            let peer_list = clients.iter().filter_map( |s| map.get(&s).cloned().unwrap()).collect();

            Ok(Response::new(PeerList { list: peer_list }))
        } else {
            // just returns an empty list
            Ok(Response::new(PeerList { list: vec![] }))
        }
    }

    async fn send_file_request(&self, request: Request<ConnectionIds>) -> Result<Response<()>, Status> {
        let r = request.into_inner();
        //this is the connection id retrieved from get_file_peer_list() of the peer seeding
        let seeder_peer_id = r.connection_peer.ok_or(Status::invalid_argument("missing peer id"))?;

        //this is your own client_id so they can find your connection id
        let self_id = r.self_id.ok_or(Status::invalid_argument("missing self"))?;

        //loop through 3 times after delay.
        //this just gives multiple attempts in case of network delay or seeding at bad times
        for i in 0 ..3 {
            if let Some(tx) = self.seed_notifier.write().await.remove(&seeder_peer_id) {
                tx.send(self_id).await.map_err(|_| Status::internal("Failed to send self_id"))?;
                break;
            } else {
                sleep(Duration::from_millis(250)).await;
                if i == 2 {
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
    //todo consider renaming
    async fn seed(&self, request: Request<PeerId>) -> Result<Response<PeerId>, Status> {
        let self_peer_id = request.into_inner();
        
        let (peer_id_tx, mut peer_id_rx) = mpsc::channel::<PeerId>(1);
        
        self.seed_notifier.write().await.insert(self_peer_id, peer_id_tx);
        
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

        self.init_hole_punch.write().await.insert(self_id, watch_tx);

        watch_rx.changed().await.map_err(|e| Status::new(Code::Internal, e.to_string()))?;
        
        Ok(Response::new(()))
        
    }
    
    /// init_hole_punch() is used to notify a seeding peer that they should begin the udp hole punching procedure.
    /// This function should be called right before the calling peer initiates their own hole punching procedure
    /// as UDP hole punching is time-sensitive.
    async fn init_punch(&self, request: Request<PeerId>) -> Result<Response<()>, Status> {
       
        let peer_id = request.into_inner();
  
        for i in 0..3 {
            match self.init_hole_punch.read().await.get(&peer_id) {
                Some(notify_handle) => {
                    println!("Hole Punch notifier received by Leecher");
                    notify_handle.send(true).map_err(|e| Status::internal(e.to_string()))?;
                    break;
                },
                None => {
                    Err(Status::internal("no seeding peer"))?;
                    sleep(Duration::from_millis(250)).await;
                    if i == 2 {
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

        self.file_tracker.write().await.insert(file_hash.clone(), info_hash);
        
        self.seeder_list.write().await
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
        _request: Request<()>
    ) -> Result<Response<FileList>, Status> {
        
        let info_hashes = self.file_tracker.read().await
            .iter()
            // get all the values (1) in the map
            .map(|x| x.1.clone()).collect::<Vec<_>>();
        
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