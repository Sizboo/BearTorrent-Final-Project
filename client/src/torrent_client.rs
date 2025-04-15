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
    pub async fn send_data(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let request = tonic::Request::new(ConnReq {
            ipaddr: 1234,
            port: 111,
            info: "lots of cool info".to_string(),
        });

        let response = self.client.send_data(request).await?.into_inner();
        println!("{:?}", response);
        
        Ok(())
    }

}
