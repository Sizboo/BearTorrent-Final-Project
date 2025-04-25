mod torrent_client;

use torrent_client::TorrentClient;

use tonic::transport::{Channel, ClientTlsConfig};


pub mod connection {
    tonic::include_proto!("connection");
}

const GCLOUD_URL: &str = "https://helpful-serf-server-1016068426296.us-south1.run.app:";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    //tls config
    //webki roots uses Mozilla's certificate store
    let tls = ClientTlsConfig::new()
        .with_webpki_roots()
        .domain_name("helpful-serf-server-1016068426296.us-south1.run.app");

    let endpoint = Channel::from_static(GCLOUD_URL).tls_config(tls)?
        .connect().await?;

    let torrent_client = TorrentClient::new(endpoint);

    let _ = torrent_client.seeding().await?;
    

    

Ok(())

}
