mod torrent_client;
mod server_connection;
mod quic_p2p_sender;
mod turn_fallback;
mod connection;
mod file_handler;
mod piece_assembler;
mod file_assembler;
mod message;

use std::collections::HashMap;
use torrent_client::TorrentClient;
use crate::file_handler::InfoHash;
use crate::server_connection::ServerConnection;



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    rustls::crypto::CryptoProvider::install_default(rustls::crypto::ring::default_provider()).expect("cannot install default provider");

    let mut server_conn = ServerConnection::new().await?;


    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let command = input.trim();
    // let server_conn_clone = server_conn.clone();

    match command {
        "s" => {
            println!("Seeding");

            let mut server_clone = server_conn.clone();
            let seeding = tokio::spawn( async move {
                TorrentClient::seeding(&mut server_clone).await.unwrap();
            });

            seeding.await.expect("seeding broken");
        }
        "r" => {
            println!("Requesting");
            //todo will need have a requesting process like seeding above
            let mut torrent_client = server_conn.register_new_client().await?;
            
            let files = torrent_client.get_server_files().await?;
            
            let mut file_selection: HashMap<u16, InfoHash> = HashMap::new();
            
            let mut i :u16 = 0;
            
            for file in files {
                println!("Option: {} -> File: {}", i, file.name);
                file_selection.insert(i, file);
                i += 1;
            }
            
            println!("\n\n type a number for your selection:");

            std::io::stdin().read_line(&mut input)?;
            let command = input.trim().parse::<u16>().expect("invalid number");
            
            let file_requested = file_selection.get(&command).unwrap();
            println!("You Requested: {}", file_requested.name);
            
            torrent_client.file_request(InfoHash::get_server_info_hash(file_requested)).await?;
            
            

        }
        _ => {
            println!("Unknown command: {}", command);
        }

    }


    Ok(())

}
