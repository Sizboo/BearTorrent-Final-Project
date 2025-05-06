mod torrent_client;
mod server_connection;
mod quic_p2p_sender;
mod turn_fallback;
mod connection;
mod file_handler;
mod data_router;
mod piece_assembler;
mod message;

use torrent_client::TorrentClient;

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
            let mut torrent_client = TorrentClient::new(&mut server_conn).await?;

            //todo I really need to change how this is done
            let _ = server_conn.register_server_connection(torrent_client.self_addr);

            let file_hash = 1234;

            let mut peer_list = torrent_client.file_request(server_conn.uid.unwrap(), file_hash).await?;

            torrent_client.get_file_from_peer(peer_list.list.pop().unwrap()).await?;
        }
        _ => {
            println!("Unknown command: {}", command);
        }

    }


    Ok(())

}
