mod torrent_client;

use stunclient::StunClient;
use std::net::{ToSocketAddrs, UdpSocket};
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

    // let endpoint = Channel::from_static(GCLOUD_URL).tls_config(tls)?
    //     .connect().await?;
    // 
    // let torrent_client = TorrentClient::new(endpoint);
    // 
    // let res = torrent_client.test_ip().await;
    // 
    // if res.is_err() {
    //     println!("{}", res.err().unwrap().to_string());
    // }
    
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    
    let stun_server = "stun.l.google.com:19302".to_socket_addrs().unwrap().filter(|x|x.is_ipv4()).next().unwrap();
    
    let client = StunClient::new(stun_server);
    
    let external_addr = client.query_external_address(&socket)?;

    println!("external ip: {:?}", external_addr.ip());
    println!("external port: {:?}", external_addr.port());
    
    loop {
    
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        let command = input.trim();
        
        match command {
            "s" => {
                println!("Seeding");
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
