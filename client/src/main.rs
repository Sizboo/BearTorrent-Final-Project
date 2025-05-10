#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod demo;
mod torrent_client;
mod server_connection;
mod quic_p2p_sender;
mod turn_fallback;
mod connection;
mod file_handler;

use tauri::Manager;
use torrent_client::TorrentClient;
use server_connection::ServerConnection;

#[tauri::command]
async fn say_hello() -> String {
    println!("say_hello() was called!");
    "Hello from Rust!".to_string()
}

#[tauri::command]
async fn start_seeding() -> Result<(), String> {
    println!("Seeding...");

    let mut server_conn = ServerConnection::new()
        .await
        .map_err(|e| format!("Failed to connect: {:?}", e))?;

    let mut server_clone = server_conn.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = TorrentClient::seeding(&mut server_clone).await {
            eprintln!("Seeding failed: {:?}", e);
        }
    });

    Ok(())
}

#[tauri::command]
async fn start_requesting() -> Result<(), String> {
    println!("Requesting...");

    let mut server_conn = ServerConnection::new()
        .await
        .map_err(|e| format!("Failed to connect: {:?}", e))?;

    let mut torrent_client = TorrentClient::new(&mut server_conn)
        .await
        .map_err(|e| format!("Failed to create client: {:?}", e))?;

    if let Some(uid) = server_conn.uid {
        let _ = server_conn.register_server_connection(torrent_client.self_addr);

        let file_hash = 1234;
        let mut peer_list = torrent_client
            .file_request(uid, file_hash)
            .await
            .map_err(|e| format!("Failed to request file: {:?}", e))?;

        if let Some(peer) = peer_list.list.pop() {
            torrent_client
                .get_file_from_peer(peer)
                .await
                .map_err(|e| format!("File download failed: {:?}", e))?;
        } else {
            return Err("No peers available".to_string());
        }
    } else {
        return Err("UID missing".to_string());
    }

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            say_hello,
            start_seeding,
            start_requesting
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
