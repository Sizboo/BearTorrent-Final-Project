import { invoke } from '@tauri-apps/api/tauri';

export async function sayHello() {
try {
const message = await window.__TAURI__.invoke("say_hello");
return message;
} catch (error) {
console.error("Failed to invoke say_hello:", error);
return "Error!";
}
}
