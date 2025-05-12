mod demo;
use demo::{say_hello, say_hello_delayed, download, get_available_files, delete_file, start_seeding, stop_seeding, reconnect, disconnect, is_connected, is_seeding};
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
                        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
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
            is_seeding,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
