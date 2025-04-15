use tonic::transport::{Channel, ClientTlsConfig};
use connection::{ConnReq, connector_client::ConnectorClient};

pub mod connection {
    tonic::include_proto!("connection");
}

const GCLOUD_URL: &str = "https://helpful-serf-server-1016068426296.us-south1.run.app";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    //tls config
    //webki roots uses Mozilla's certificate store
    let tls = ClientTlsConfig::new()
        .with_webpki_roots()
        .domain_name("helpful-serf-server-1016068426296.us-south1.run.app");

    let endpoint = Channel::from_static(GCLOUD_URL).tls_config(tls)?
        .connect().await?;

    let mut client = ConnectorClient::new(endpoint);

    let request = tonic::Request::new(ConnReq {
        ipaddr: 1234,
        port: 111,
        info: "lots of cool info".to_string(),
    });
    
    let response = client.send_data(request).await?.into_inner();
    println!("{:?}", response);

   Ok(()) 

}
