
use serde::{Serialize, Deserialize};
use crate::connection::connection::InfoHash;
use sha1::{Sha1, Digest};
use hex;
pub mod connection {
    tonic::include_proto!("connection");
}


pub fn hash_infohash(proto: &InfoHash) -> String {
    let mut hasher = Sha1::new();

    hasher.update(proto.name.as_bytes());
    hasher.update(&proto.file_length.to_be_bytes());
    hasher.update(&proto.piece_length.to_be_bytes());

    for piece in &proto.pieces {
        hasher.update(&piece.hash); // assuming `piece.hash: [u8; 20]`
    }

    let result = hasher.finalize();
    hex::encode(result)
}


#[derive(Debug, Serialize, Deserialize)]
pub struct SerializableFileInfo {
    pub name: String,
    pub size: f64,            // in MB
    pub last_modified: String,
    pub file_type: String,
    pub hash: String,
}

impl From<InfoHash> for SerializableFileInfo {
    fn from(proto: InfoHash) -> Self {
        let file_type = if proto.name.ends_with(".pdf") {
            "PDF"
        } else if proto.name.ends_with(".jpg") || proto.name.ends_with(".png") {
            "Image"
        } else if proto.name.ends_with(".ppt") || proto.name.ends_with(".pptx") {
            "Presentation"
        } else if proto.name.ends_with(".txt") {
            "Text"
        } else {
            "Folder"
        }.to_string();

        let hash = crate::connection::hash_infohash(&proto);

        Self {
            name: proto.name.clone(),
            size: proto.file_length as f64 / 1_000_000.0,
            last_modified: "Unknown".into(),
            file_type,
            hash,
        }
    }
}

/*


use serde::{Serialize, Deserialize};
use crate::connection::connection::InfoHash;
pub mod connection {
    tonic::include_proto!("connection");
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializableFileInfo {
    pub name: String,
    pub size: f64,            // in MB
    pub last_modified: String,
    pub file_type: String,
    pub info_name: String,
    pub info_file_length: u64,
    pub info_piece_length: u32,
    pub info_pieces: Vec<connection::PieceHash>
}

impl From<InfoHash> for SerializableFileInfo {
    fn from(proto: InfoHash) -> Self {
        // You may want to infer file type from name/extension:
        let file_type = if proto.name.ends_with(".pdf") {
            "PDF"
        } else if proto.name.ends_with(".jpg") || proto.name.ends_with(".png") {
            "Image"
        } else if proto.name.ends_with(".ppt") || proto.name.ends_with(".pptx") {
            "Presentation"
        } else if proto.name.ends_with(".txt") {
            "Text"
        } else {
            "Folder"
        }.to_string();

        // Optionally hardcode or compute these if not present in InfoHash
        Self {
            name: proto.name.clone(),
            size: proto.file_length as f64 / 1_000_000.0, // bytes → MB
            last_modified: "Unknown".into(), // You can update this later
            file_type,
            info_name: proto.name,
            info_file_length: proto.file_length,
            info_piece_length: proto.piece_length,
            info_pieces: proto.pieces,
        }
    }
}

impl From<SerializableFileInfo> for InfoHash {
    fn from(sfi: SerializableFileInfo) -> Self {
        Self{
            name: sfi.info_name,
            file_length: sfi.info_file_length,
            piece_length: sfi.info_piece_length,
            pieces: sfi.info_pieces,
        }
    }
}

 */