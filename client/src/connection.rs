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
        }
    }
}
