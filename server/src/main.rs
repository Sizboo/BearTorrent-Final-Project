use std::{env, collections::HashMap, net::{IpAddr, SocketAddr, Ipv4Addr}, sync::Arc};
use tonic::{transport::Server, Code, Request, Response, Status, Streaming};
use connection::{PeerId, PeerList, FileMessage, TurnPair, connector_server::{Connector, ConnectorServer}, turn_server::Turn};
use tokio::{sync::{Mutex, mpsc}, net::UdpSocket};
use crate::connection::ClientId;

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

        let mut file_tracker = self.file_tracker.lock().await;

        // notify all seeders of the file
        if let Some(seeders) = file_tracker.get_mut(&r.info_hash) {
            //todo implement this so that it selects specific peers (or pieces out file)
            for seeder in seeders.iter_mut() {
                let _ = seeder.notify.send(requester.clone()).await;
            }

            // returns a list of all peers that have a file
            //todo this needs to send THE seeder/s that are sharing for hole punching, not necessarily everyone with file
            let peer_list = seeders.iter().map(|s| s.peer.clone()).collect();
            Ok(Response::new(PeerList { list: peer_list }))
        } else {
            // just returns an empty list
            Ok(Response::new(PeerList { list: vec![] }))
        }
    }

    ///this function is used by clients willing to share data to get peers who request data.
    /// clients should listen to this service at all times they are willing to send.
    //todo consider renaming
    async fn get_peer(&self, request: Request<PeerId>) -> Result<Response<PeerId>, Status> {
        println!("get_peer called");
        let peer_id = request.into_inner();
        
        //todo if we implement states (offline, seeding) should first update its state on server to sharing
        // any time in offline status it will not be selected


        match self.send_tracker.lock().await.get_mut(&peer_id) {
            Some(recv) => {
                let peer_id = recv.recv().await.ok_or(Status::new(Code::Internal, "peer id not returned upon signal from server"))?;
                Ok(Response::new(peer_id))
            }
            None => Err(Status::internal("dropped")),
        }


    }

    /// this function is used to advertise a client owns a file that can be shared 
    async fn advertise(
        &self,
        request: Request<FileMessage>,
    ) -> Result<Response<PeerId>, Status> {
        let r = request.into_inner();

        println!("advertising file: {:}", r.info_hash);

        let peer_id = match r.id {
            Some(id) => id,
            None => return Err(Status::invalid_argument("PeerId missing")),
        };

        let (tx, rx) = mpsc::channel(1);

        let mut file_tracker = self.file_tracker.lock().await;
        file_tracker.entry(r.info_hash).or_default().push(Seeder {
            peer: peer_id.clone(),
            notify: tx,
        });

        let mut send_tracker = self.send_tracker.lock().await;
        send_tracker.insert(peer_id, rx);

        Ok( Response::new(peer_id) )
    }

    async fn register_client(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ClientId>, Status> {


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



type RelayKey = (SocketAddr, SocketAddr);

pub struct TurnService {}

impl TurnService {
    async fn relay_loop (
        socket: Arc<UdpSocket>,
        from: SocketAddr,
        to: SocketAddr,
    ) {
        let mut buf = [0u8; 1500];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, src)) => {
                    if src == from {
                        if let Err(e) = socket.send_to(&buf[..n], to).await {
                            eprintln!("Error sending to {}: {}", to, e);
                        }
                    } else {
                        eprintln!("Unexpected source: expected {}, got {}", from, src);
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving from {}: {}", from, e);
                    break;
                }
            }
        }
    }
}

#[tonic::async_trait]
impl Turn for TurnService {
    async fn establish_relay(
        &self,
        request: Request<TurnPair>,
    ) -> Result<Response<()>, Status> {
        let pair = request.into_inner();

        let sender = pair.sender.ok_or_else(|| Status::invalid_argument("missing sender"))?;
        let receiver = pair.receiver.ok_or_else(|| Status::invalid_argument("missing receiver"))?;

        let sender_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from(sender.ipaddr)), sender.port as u16);
        let receiver_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from(receiver.ipaddr)), receiver.port as u16);

        let socket = UdpSocket::bind("0.0.0.0:0").await
            .map_err(|e| Status::internal(format!("Socket bind failed: {e}")))?;

        let socket = Arc::new(socket);
        let socket_sender = socket.clone();
        let socket_receiver = socket.clone();

        tokio::spawn(TurnService::relay_loop(socket_sender, sender_addr, receiver_addr));

        Ok(Response::new(()))
    }
}