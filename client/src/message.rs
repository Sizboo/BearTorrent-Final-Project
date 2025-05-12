
#[derive(Debug, PartialEq)]
#[repr(u8)]
// Using the message IDs and taking descriptions from the specification.
pub enum Message{
    // Fixed length message used to request a block from a piece.
    // If pieces are large, a request on the same piece could be
    // sent with successive 'begin' values
    Request{
        seeder: u32, // this is the seeder ndx so leecher knows which seeder this is
        index: u32, // Pieces are requested by their zero-based index value
        begin: u32, // The zero-based byte offset within the piece being requested
        length: u32, // Requested length to get from the piece
        hash: [u8; 20],
    } = 6,

    // Variable length message containing a block of the piece.
    Piece{
        index: u32, // Zero-based index of the piece
        piece: Vec<u8> // The block of data, which is a subset of the piece specified by the index
    } = 7,

    // Fixed length message to cancel a block request. 
    Cancel{
        seeder: u32, // this is seeder ndx for leecher to manage connections
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
            Message::Request{ seeder, index, begin, length , hash} => {
                buf.extend_from_slice(&37u32.to_be_bytes()); // Message is always same length
                buf.push(6);
                buf.extend_from_slice(&seeder.to_be_bytes());
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
            Message::Cancel{ seeder, index, begin, length } => {
                buf.extend_from_slice(&17u32.to_be_bytes());
                buf.push(8);
                buf.extend_from_slice(&seeder.to_be_bytes());
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

        let message_id = buf[4];
        
        match message_id { 
            6 => {
                let seeder = u32::from_be_bytes(buf[5..9].try_into().unwrap());
                let index = u32::from_be_bytes(buf[9..13].try_into().unwrap());
                let begin = u32::from_be_bytes(buf[13..17].try_into().unwrap());
                let length = u32::from_be_bytes(buf[17..21].try_into().unwrap());
                let hash = buf[21..41].try_into().unwrap();
                Some(Message::Request{ seeder, index, begin, length, hash })
            }
            7 => {
                let index = u32::from_be_bytes(buf[5..9].try_into().unwrap());
                let piece = buf[9..].to_vec(); // TODO double check this is right
                Some(Message::Piece{ index,  piece })
            }
            8 => {
                let seeder = u32::from_be_bytes(buf[5..9].try_into().unwrap());
                let index = u32::from_be_bytes(buf[9..13].try_into().unwrap());
                let begin = u32::from_be_bytes(buf[13..17].try_into().unwrap());
                let length = u32::from_be_bytes(buf[17..21].try_into().unwrap());
                Some(Message::Cancel{ seeder, index, begin, length })
            }
            _ => {
                // TODO Other messages types only as needed
                None
            }
        }
    }
}