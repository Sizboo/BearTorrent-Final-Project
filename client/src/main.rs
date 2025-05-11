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
async fn greetings() -> String {
    println!("greetings() was called!");
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
            let seeding = tokio::spawn(async move {
                client_clone.seeding().await.unwrap();
            });
        }
    }
    Ok(())
}

#[tauri::command]
async fn start_requesting() -> Result<(), String> {
    println!("Starting requesting...");
    let mut torrent_client = TorrentClient::new().await?;
    let files = torrent_client.get;
}

fn main() {

    rustls::crypto::CryptoProvider::install_default(rustls::crypto::ring::default_provider()).expect("cannot install default provider");

    //let mut torrent_client = TorrentClient::new().await?;


    //let mut input = String::new();
    //std::io::stdin().read_line(&mut input)?;
    //let command = input.trim();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            say_hello,
            start_seeding,
            start_requesting
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
