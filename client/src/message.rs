
#[derive(Debug, PartialEq)]
#[repr(u8)]
// Using the message IDs and taking descriptions from the specification.
pub enum Message{
    // Fixed length message used to request a block from a piece.
    // If pieces are large, a request on the same piece could be
    // sent with successive 'begin' values
    Request{
        index: u32, // Pieces are requested by their zero-based index value
        begin: u32, // The zero-based byte offset within the piece being requested
        length: u32, // Requested length to get from the piece
        hash: [u8; 20],
    } = 6,

    // Variable length message containing a block of the piece.
    Piece{
        index: u32, // Zero-based index of the piece
        // begin: u32, // The zero-based byte offset within the piece
        piece: Vec<u8> // The block of data, which is a subset of the piece specified by the index
    } = 7,

    // Fixed length message to cancel a block request. Payload is identical to the request message.
    Cancel{
        index: u32, // Zero-based index of the piece
        begin: u32, // Zero-based byte offset within the piece
        length: u32 // Requested length of the piece
    } = 8,
}

impl Message{
    
    // Encodes the message 
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        match self {
            Message::Request{ index, begin, length , hash} => {
                buf.extend_from_slice(&33u32.to_be_bytes()); // Message is always same length
                buf.push(6);
                buf.extend_from_slice(&index.to_be_bytes());
                buf.extend_from_slice(&begin.to_be_bytes());
                buf.extend_from_slice(&length.to_be_bytes());
                buf.extend_from_slice(hash);
            }
            Message::Piece{ index,  piece } => {
                buf.extend_from_slice((5 + piece.len() as u32).to_be_bytes().as_ref());
                buf.push(7);
                buf.extend_from_slice(&index.to_be_bytes());
                buf.extend_from_slice(piece);
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
    pub fn decode(buf: Vec<u8>) -> Option<Message> {
        if buf.len() < 4 {
            return None;
        }

        let length = u32::from_be_bytes(buf[0..4].try_into().unwrap()) as usize;
        
        let message_id = buf[4];
        
        match message_id { 
            6 => {
                let index = u32::from_be_bytes(buf[5..9].try_into().unwrap());
                let begin = u32::from_be_bytes(buf[9..13].try_into().unwrap());
                let length = u32::from_be_bytes(buf[13..17].try_into().unwrap());
                let hash = buf[17..37].try_into().unwrap();
                Some(Message::Request{ index, begin, length, hash })
            }
            7 => {
                let index = u32::from_be_bytes(buf[5..9].try_into().unwrap());
                let piece = buf[9..].to_vec(); // TODO double check this is right
                Some(Message::Piece{ index,  piece })
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