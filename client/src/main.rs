use connection::{ConnReq, connector_client::ConnectorClient};

pub mod connection {
    tonic::include_proto!("connection");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "https://helpful-serf-server-1016068426296.us-south1.run.app";
    let mut client = ConnectorClient::connect(addr).await?;
    
    let request = tonic::Request::new(ConnReq {
        ipaddr: 1234,
        port: 111,
        info: "lots of cool info".to_string(),
    });
    
    let response = client.send_data(request).await?.into_inner();
    println!("{:?}", response);

   Ok(()) 

}
