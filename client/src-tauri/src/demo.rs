use tauri::command;
use std::time::Duration;
use tokio::time::sleep;
use crate::AppState;
use client::connection::SerializableFileInfo;
use tauri::State;



#[command]
pub fn say_hello() -> String {
    println!("say_hello() was called!");
    "Hello World".to_string()
}

#[tauri::command]
pub async fn download(hash: String, state: State<'_, AppState>) -> Result<String, String> {
    let mut client = state.client.write().await;

    let server_files = client
        .get_server_files()
        .await
        .map_err(|e| format!("Failed to get files: {}", e))?;

    if let Some(info_hash) = server_files.into_iter().find(|f| {
        let computed_hash = client::connection::hash_infohash(f);
        computed_hash == hash
    }) {
        client
            .file_request(info_hash)
            .await
            .map_err(|e| format!("Failed to request file: {}", e))?;

        Ok(format!("Download started for hash: {}", hash))
    } else {
        Err(format!("No file matched the given hash: {}", hash))
    }
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

