import { invoke } from '@tauri-apps/api/tauri';

export async function sayHello() {
    try {
        const message = await invoke("say_hello");
        return message;
    } catch (error) {
        console.error("Rust invoke failed:", error);
        return "Error!";
    }
}
