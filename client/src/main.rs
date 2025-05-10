#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod demo;
mod peer_connection;
mod torrent_client;
mod quic_p2p_sender;
mod turn_fallback;
mod connection;
mod file_handler;
mod piece_assembler;
mod file_assembler;
mod message;

use std::collections::HashMap;
use std::io::Write;
use peer_connection::PeerConnection;
use crate::connection::connection::InfoHash;
use crate::torrent_client::TorrentClient;


use tauri::Manager;

#[tauri::command]
async fn say_hello() -> String {
    println!("say_hello() was called!");
    "Hello from Rust!".to_string()
}

#[tauri::command]
async fn start_seeding() -> Result<(), String> {
    println!("Seeding...");

    let mut torrent_client = TorrentClient::new().await?;


    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let command = input.trim();
    // let server_conn_clone = server_conn.clone();

    match command {
        "s" => {
            println!("Seeding");

            let mut client_clone = torrent_client.clone();
            let seeding = tokio::spawn( async move {
                client_clone.seeding().await.unwrap();
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
