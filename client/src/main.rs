mod torrent_client;

use std::net::{ToSocketAddrs};
use std::sync::Arc;
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
    
    let torrent_client = Arc::new(TorrentClient::new(endpoint).await?);
    
    loop {
    
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let client_arc = Arc::clone(&torrent_client);
        
        let command = input.trim();
        
        match command {
            "s" => {
                println!("Seeding");

                client_arc.advertise().await?; 
                
                let client_arc = Arc::clone(&torrent_client);
               
                tokio::spawn( async move {
                    client_arc.seeding().await.unwrap(); 
                });
            }
            "r" => {
                println!("Requesting");
            }
            "q" => {
                println!("Quitting");
                break;
            }
            _ => {
                println!("Unknown command: {}", command);
            }
            
        }
    }
    

Ok(())

}
