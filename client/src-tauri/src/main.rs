use client::demo;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            demo::say_hello
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri");
}
