// use crate::connection::connection::*;
// use crate::message::Message;
// use std::collections::HashMap;
// use tokio::sync::mpsc;
// 
// /// PieceAssembler is the tool we will use to form pieces out of incoming packets (blocks)
// pub struct PieceAssembler {
//     // todo is it even possible to get this? ask Sam
//     /// total length of the piece
//     piece_length: u32,
//     /// length of the blocks
//     block_size: u32,
//     /// storage buffer for piece. blocks we have: Some<Vec<u8>>, blocks we don't are: None
//     buf: HashMap<u32, Vec<Option<Vec<u8>>>>,
// }
// 
// impl PieceAssembler {
//     /// function to instantiate a new PieceAssembler for a piece
//     pub fn new(piece_length: u32, block_size: u32) -> Self {
//         PieceAssembler {
//             piece_length,
//             block_size,
//             buf: HashMap::new(),
//         }
//     }
// 
//     // todo function to add block to piece
//     pub async fn assemble(
//         &mut self,
//         rx: &mut mpsc::Receiver<Message>,
//     ) -> Vec<u8> {
//         let mut buffer = Vec::new();
//         while let Some(msg) = rx.recv().await {
//             if let Message::Block { block, .. } = msg {
//                 buffer.extend(block);
//             }
//         }
//         buffer
//     }
// }