use client::demo;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            demo::say_hello,
            demo::download,
            demo::say_hello_delayed,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri");
}
