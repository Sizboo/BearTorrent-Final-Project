use tauri::command;
use std::time::Duration;
use tokio::time::sleep;
use crate::AppState;
use client::connection::SerializableFileInfo;
use tauri::State;
use client::torrent_client::TorrentClient;



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
pub async fn delete_file(hash: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let mut client = state.client.write().await;

    let server_files = client
        .get_server_files()
        .await
        .map_err(|e| format!("Failed to fetch server files: {}", e))?;

    if let Some(info_hash) = server_files.into_iter().find(|f| {
        let computed_hash = client::connection::hash_infohash(f);
        computed_hash == hash
    }) {
        client
            .delete_file(info_hash)
            .await
            .map_err(|e| format!("Failed to delete file: {}", e))?;

        Ok(format!("File deleted for hash: {}", hash))
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

#[tauri::command]
pub async fn start_seeding(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut flag = state.is_seeding.write().await;
    *flag = true;
    Ok(())
}

#[tauri::command]
pub async fn stop_seeding(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut flag = state.is_seeding.write().await;
    *flag = false;
    Ok(())
}

/// Fully disconnects the client from the tracker (graceful shutdown).
#[tauri::command]
pub async fn disconnect(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let client = state.client.read().await;
    client.remove_client().await?;
    Ok(())
}

#[tauri::command]
pub async fn reconnect(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let new_client = TorrentClient::new().await?;
    let mut client = state.client.write().await;
    *client = new_client;
    Ok(())
}


#[tauri::command]
pub async fn is_connected(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let client = state.client.read().await;
    Ok(client.is_connected())
}


