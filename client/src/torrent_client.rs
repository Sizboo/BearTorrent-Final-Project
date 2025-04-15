use tonic::transport::{Channel};
use connection::{ConnReq, connector_client::ConnectorClient};

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
        
        
        
        let request = tonic::Request::new(ConnReq {
            ipaddr: 1234,
            port: 111,
            info: "lots of cool info".to_string(),
        });

        let response = client.send_data(request).await?.into_inner();
        println!("{:?}", response);
        
        Ok(())
    }
    
    ///seeding is used as a listening process to begin sending data upon request
    /// it simply awaits a server request for it to send data
    pub async fn seeding(self) -> Result<(), Box<dyn std::error::Error>> { 
        let client = self.client.clone(); 
        //todo loop through client.get_peer() invocation
        
        let handle = tokio::spawn(async move {
            //todo: provide error checking and fail safe default
            let _ = self.send_data().await;
        });
        
        handle.await?;
        
        Ok(())
    }

}
