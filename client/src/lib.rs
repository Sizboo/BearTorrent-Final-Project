pub mod peer_connection;
pub mod torrent_client;
pub mod quic_p2p_sender;
pub mod turn_fallback;
pub mod connection;
pub mod file_handler;
pub mod piece_assembler;
pub mod file_assembler;

pub mod message;
use std::sync::Arc;
use tokio::sync::RwLock;
pub use crate::connection::SerializableFileInfo;

pub use crate::torrent_client::TorrentClient;

pub struct AppState {
    pub client: Arc<RwLock<TorrentClient>>,
    pub is_seeding: Arc<RwLock<bool>>,
}


