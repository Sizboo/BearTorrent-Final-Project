use tauri::command;

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
