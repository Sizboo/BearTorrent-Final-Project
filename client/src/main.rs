mod torrent_client;

use std::net::{ToSocketAddrs};
use std::sync::Arc;
use torrent_client::TorrentClient;

use tonic::transport::{Channel, ClientTlsConfig};
use crate::torrent_client::ServerConnection;

pub mod connection {
    tonic::include_proto!("connection");
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {


    let server_conn = ServerConnection::new().await?;

    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        let command = input.trim();
        let server_conn_clone = server_conn.clone();

        match command {
            "s" => {
                println!("Seeding");

                tokio::spawn( async move {
                    TorrentClient::seeding(server_conn_clone).await.unwrap();
                });
            }
            "r" => {
                println!("Requesting");
                //todo will need have a requesting process probably
                let mut torrent_client = TorrentClient::new(server_conn_clone).await?;

                let file_hash = 12345;

                let mut peer_list = torrent_client.file_request(file_hash).await?;

                torrent_client.get_file_from_peer(peer_list.list.pop().unwrap()).await?;
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
