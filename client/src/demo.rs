use tauri::command;
use std::time::Duration;
use tokio::time::sleep;
use crate::AppState;
use crate::connection::SerializableFileInfo;
use tauri::State;



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

#[command]
pub async fn get_available_files(state: State<'_, AppState>) -> Result<Vec<SerializableFileInfo>, String> {
    let client = state.client.read().await;
    match client.get_server_files().await {
        Ok(files) => Ok(files.into_iter().map(Into::into).collect()),
        Err(e) => Err(format!("Error getting files: {}", e)),
    }
}

