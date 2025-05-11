use tauri::command;
use std::time::Duration;
use tokio::time::sleep;
use crate::torrent_client::TorrentClient;
use crate::connection::connection::InfoHash;
use crate::SerializableFileInfo;
use tauri::Manager;
use std::sync::Mutex;



#[command]
pub fn say_hello() -> String {
    println!("say_hello() was called!");
    "Hello World".to_string()
}

#[command]
pub fn download() -> String {
    println!("Simulated download() called.");
    "Download simulated successfully".to_string()
}


#[tauri::command]
pub async fn say_hello_delayed() -> String {
    println!("[Rust] Starting delay...");
    sleep(Duration::from_secs(2)).await;
    println!("[Rust] Finished delay!");
    "Hello after 2 seconds from Rust!".into()
}

#[tauri::command]
pub async fn get_available_files(state: tauri::State<'_, Mutex<AppState>>) -> Result<Vec<SerializableFileInfo>, String> {
    let mut state = state.lock();
    match state {
        Ok(app_state) => match app_state.get_server_files().await {
            Ok(files) => Ok(files.into_iter().map(|f| f.into()).collect()),
            Err(e) => Err(format!("Error getting files: {}", e)),
        },
        Err(e) => Err(format!("Failed to acquire lock: {}", e)),
    }
}


pub struct AppState {
    pub client: std::sync::Arc<tokio::sync::RwLock<TorrentClient>>,
}