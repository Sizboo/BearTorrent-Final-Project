use tonic::Request;
use tonic::transport::{Channel};
use connection::{PeerId, connector_client::ConnectorClient};
use crate::connection::FileMessage;

pub mod connection {
    tonic::include_proto!("connection");
}


pub struct TorrentClient {
    client: ConnectorClient<Channel>
}

impl TorrentClient {

    pub fn new(channel: Channel) -> Self {
        TorrentClient {
            client: ConnectorClient::new(channel)
        }
    }

    ///init is used to initialize and maintain a connection to the torrent server
    /// init MUST be called
    /// it will need to initialize a connection and send a periodic announcement so server knows it is an available seeder
    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        Ok(())
    }

    /// send_data is used to send data to peer
    /// Process:
    /// parameters - peer ip and port
    /// return Ok() upon completion Err() if connection cannot be made or data send fail
    ///
    /// it will need to try UDP hole punching with the given parameter
    /// then send data
    pub async fn send_data(self) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = self.client.clone();

       //todo actual send logic to other peer

        // let request = tonic::Request::new(connection::FileMessage {
        //     id: Some(connection::PeerId {
        //         ipaddr: 1234,
        //         port: 8080,
        //     }),
        //     info_hash: 12345,
        // });
        // 
        // let response = client.send_file_request(request).await?.into_inner();
        // println!("{:?}", response);

        Ok(())
    }

    ///seeding is used as a listening process to begin sending data upon request
    /// it simply awaits a server request for it to send data
    pub async fn seeding(self, self_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = self.client.clone();
        
        loop {
            // calls get_peer
            let response = client.get_peer(self_id).await;

            // waits for response from get_peer
            match response {
                Ok(res) => {
                    let peer_id = res.into_inner();
                    
                    println!("peer seeding: {:?}", peer_id);
                    
                    // tokio::spawn(async move {
                    //     // TODO spawn send_data process here
                    // });
                }
                Err(e) => {
                    eprintln!("Failed to get peer: {:?}", e);
                }
            }
        }
    }

    ///request is a method used to request necessary connection details from the server
    pub async fn file_request(self, self_id: PeerId, file_hash: u32) -> Result<connection::PeerList, Box<dyn std::error::Error>> {
        let mut client = self.client.clone();
        
        
        let request = Request::new(connection::FileMessage {
            id: Some(self_id),
            info_hash: file_hash,
        });
        
        let resp = client.send_file_request(request).await?;
        
        Ok(resp.into_inner())
    }

    ///download is a process used to retrieve data from a peer
    /// parameters:
    /// peer ip and port
    ///
    /// download will initiate UDP hole punching from receiving end
    ///  then receive data
    pub async fn download(self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }


    /// announce is a method used update server with connection details
    /// this will primarily be used by init() and called on a periodic basis
    /// this is essentially a "keep alive" method
    pub async fn announce(self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    pub async fn advertise(self, self_id: PeerId) -> Result<PeerId, Box<dyn std::error::Error>> {
        let mut client = self.client.clone();
        

        let request = Request::new(connection::FileMessage {
            id: Some(self_id),
            info_hash: 12345,
        });
        
        let resp = client.advertise(request).await?;
        
        Ok(resp.into_inner())
    }
    
    pub async fn test_ip(self) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = self.client.clone();
        
        let resp = client.test_func(Request::new(connection::ClientId {client_id: 123})).await?;
        
        println!("Client Received {:?}", resp);
       
        Ok(())
    }

}
