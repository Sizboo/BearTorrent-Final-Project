use std::io::{Cursor, Write};

#[derive(Debug, PartialEq)]
#[repr(u8)]
// Using the message IDs from the specification
pub enum Message{
    Request{ index: u32, begin: u32, length: u32 } = 6, 
    Piece{ index: u32, begin: u32, block: Vec<u8> } = 7,
    Cancel{ index: u32, begin: u32, length: u32 } = 8,
}

impl Message{
    
    // Encodes the message 
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        match self {
            Message::Request{ index, begin, length } => {
                buf.extend_from_slice(&13u32.to_be_bytes()); // Message is always same length
                buf.push(6);
                buf.extend_from_slice(&index.to_be_bytes());
                buf.extend_from_slice(&begin.to_be_bytes());
                buf.extend_from_slice(&length.to_be_bytes());
            }
            Message::Piece{ index, begin, block } => {
                buf.extend_from_slice((9 + block.len() as u32).to_be_bytes().as_ref());
                buf.push(7);
                buf.extend_from_slice(&index.to_be_bytes());
                buf.extend_from_slice(&begin.to_be_bytes());
                buf.extend_from_slice(block);
            }
            Message::Cancel{ index, begin, length } => {
                buf.extend_from_slice(&13u32.to_be_bytes());
                buf.push(8);
                buf.extend_from_slice(&index.to_be_bytes());
                buf.extend_from_slice(&begin.to_be_bytes());
                buf.extend_from_slice(&length.to_be_bytes());
            }
        }

        buf
    }
    
    // Decodes the message
    pub fn decode(buf: &[u8]) -> Option<Message> {
        if buf.len() < 4 {
            return None;
        }
        
        let mut cursor = Cursor::new(buf);
        let length = u32::from_be_bytes(buf[0..4].try_into().unwrap()) as usize;
        
        let message_id = buf[4];
        
        match message_id { 
            6 => {
                let index = u32::from_be_bytes(buf[5..9].try_into().unwrap());
                let begin = u32::from_be_bytes(buf[9..13].try_into().unwrap());
                let length = u32::from_be_bytes(buf[13..17].try_into().unwrap());
                Some(Message::Request{ index, begin, length })
            }
            7 => {
                let index = u32::from_be_bytes(buf[5..9].try_into().unwrap());
                let begin = u32::from_be_bytes(buf[9..13].try_into().unwrap());
                let block = buf[13..(4 + length)].to_vec(); // TODO double check this is right
                Some(Message::Piece{ index, begin, block })
            }
            8 => {
                let index = u32::from_be_bytes(buf[5..9].try_into().unwrap());
                let begin = u32::from_be_bytes(buf[9..13].try_into().unwrap());
                let length = u32::from_be_bytes(buf[13..17].try_into().unwrap());
                Some(Message::Cancel{ index, begin, length })
            }
            _ => {
                // TODO Other messages types only as needed
                None
            }
        }
        
        
    }
}