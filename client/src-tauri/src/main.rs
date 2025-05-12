mod demo;
use demo::{say_hello, say_hello_delayed, download, get_available_files, delete_file, start_seeding, stop_seeding, reconnect, disconnect, is_connected};
use tauri::{Manager, Emitter};
use std::sync::Arc;
use tokio::sync::RwLock;

use client::{TorrentClient, AppState, SerializableFileInfo};



fn main() {
    rustls::crypto::CryptoProvider::install_default(
        rustls::crypto::ring::default_provider()
    ).expect("cannot install default provider");


    tauri::Builder::default()
        .setup(|app| {
            let client = tauri::async_runtime::block_on(async {
                TorrentClient::new()
                    .await
                    .map_err(|e| tauri::Error::Setup(Box::<dyn std::error::Error + 'static>::from(e).into()))
            })?;

            // Store shared app state
            app.manage(AppState {
                client: Arc::new(RwLock::new(client)),
                is_seeding: Arc::new(RwLock::new(false)),
            });

            //Acquire State
            let window = app.get_webview_window("main").unwrap();
            let state: tauri::State<AppState> = app.state::<AppState>();

            let client = state.client.clone();
            let is_seeding = state.is_seeding.clone();

            // Async Task 1: Seeding toggle checker (reacts "instantly")
            tauri::async_runtime::spawn({
                async move {
                    let mut seeding_started = false;

                    loop {
                        let seeding_enabled = *is_seeding.read().await;

                        if seeding_enabled && !seeding_started {
                            let client_clone = client.clone();
                            tokio::spawn(async move {
                                let mut guard = client_clone.write().await;
                                if let Err(e) = guard.seeding().await {
                                    eprintln!("Seeding loop error: {}", e);
                                }
                            });
                            seeding_started = true;
                        }

                        if !seeding_enabled && seeding_started {
                            let guard = client.read().await;
                            if let Err(e) = guard.remove_client().await {
                                eprintln!("Failed to remove client: {}", e);
                            }
                            seeding_started = false;
                        }

                        // Check frequently for fast responsiveness
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    }
                }
            });

            // Async Task 2: File list refresher (every 30s)
            let client = state.client.clone(); // clone again to avoid move conflict
            tauri::async_runtime::spawn({
                let window = window.clone();
                async move {
                    loop {
                        let client_guard = client.read().await;
                        match client_guard.get_server_files().await {
                            Ok(files) => {
                                let serializable: Vec<SerializableFileInfo> = files.into_iter().map(Into::into).collect();
                                window
                                    .emit("file-update", &serializable)
                                    .unwrap_or_else(|e| eprintln!("emit failed: {}", e));
                            }
                            Err(e) => {
                                eprintln!("File refresh error: {}", e);
                            }
                        }

                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    }
                }
            });

            Ok(())
        }).invoke_handler(tauri::generate_handler![
            say_hello,
            download,
            say_hello_delayed,
            get_available_files,
            delete_file,
            start_seeding,
            stop_seeding,
            reconnect,
            disconnect,
            is_connected,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
