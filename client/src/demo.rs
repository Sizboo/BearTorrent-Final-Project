use tauri::command;
use std::time::Duration;
use tokio::time::sleep;

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
