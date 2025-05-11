use client::demo;
use tauri::{Manager, Emitter};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::Mutex;
use client::{TorrentClient, get_available_files, SerializableFileInfo};


fn main() {
    rustls::crypto::CryptoProvider::install_default(
        rustls::crypto::ring::default_provider()
    ).expect("cannot install default provider");


    tauri::Builder::default()
        .setup(|app| {
            // 🔧 Initialize TorrentClient with block_on (sync workaround)
            let client = tauri::async_runtime::block_on(async {
                TorrentClient::new()
                    .await
                    .map_err(|e| tauri::Error::Setup(Box::<dyn std::error::Error>::from(e).into()))



            })?;

            // Insert into AppState
            app.manage(AppState {
                client: Arc::new(Mutex::new(client)),
            });

            let window = app.get_webview_window("main").unwrap();
            let state: tauri::State<'_, AppState> = app.state::<AppState>();

            // Background polling for file updates
            tauri::async_runtime::spawn({
                let client = state.client.clone();
                async move {
                    //ALL ASYNC STUFF SHOULD BE IN HERE

                    loop {
                        // 1. Acquire read lock on the TorrentClient
                        // 2. Call get_server_files().await
                        // 3. Convert to SerializableFileInfo
                        // 4. Emit the file list to frontend
                        // 5. Sleep for 30 seconds
                        {
                            let client_guard = client.read().await;
                            match client_guard.get_server_files().await {
                                Ok(files) => {
                                    println!("[Background] Fetched {} files", files.len());

                                    //Serialize Files With conn.rs
                                    let serializable: Vec<SerializableFileInfo> = files
                                        .into_iter()
                                        .map(Into::into)
                                        .collect();
                                    //Emit New Files to Frontend
                                    window
                                        .emit("file-update", &serializable)
                                        .unwrap_or_else(|e| eprintln!("emit failed: {}", e));
                                }
                                Err(e) => {
                                    eprintln!("[Background] Failed to fetch files: {}", e);
                                }
                            }
                        }

                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            demo::say_hello,
            demo::download,
            demo::say_hello_delayed,
            demo::get_available_files(state),
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
