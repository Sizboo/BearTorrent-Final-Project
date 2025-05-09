use std::fs::File;
use std::sync::Arc;
use crate::connection::connection::*;
use crate::message::Message;
use crate::piece_assembler::*;
use tokio::sync::{mpsc, RwLock};
use crate::file_handler;
use crate::file_handler::write_piece_to_part;
use crate::peer_connection::PeerConnection;

/// this represents a connection between 2 peers
#[derive(Debug)]
pub struct FileAssembler {
    /// the sender we will use to send to a PieceAssembler
    // snd_tx: mpsc::Sender<Message>,
    /// the receiver we will let a PieceAssembler borrow so we can send to it
    // snd_rx: mpsc::Receiver<Message>,
    
    // torrent_client: &'a mut TorrentClient,
    // 
    // peer_list: Vec<PeerId>,
    
    ///the sender used for LAN/P2P/QUIC to send data from
    conn_tx: mpsc::Sender<Message>, 
    /// sender used to send file requests across a connection
    request_txs: Vec<mpsc::Sender<Message>>,
}

impl FileAssembler {
    
    pub async fn new(file_handler: file_handler::InfoHash) -> Self {
        let (conn_tx, conn_rx) = mpsc::channel::<Message>(50);
        let assembler = FileAssembler {
            conn_tx,
            request_txs: Vec::new(),
        };
        
        tokio::spawn(async move {
            Self::reassemble_loop(conn_rx, file_handler).await
        });
        
        assembler 
    }
    
    pub fn get_conn_tx(&self) -> mpsc::Sender<Message> {
        self.conn_tx.clone()
    }
    
    pub async fn send_requests(
        &self,
        num_connections: usize,
        file_hash: InfoHash
    ) -> Result<(), Box<dyn std::error::Error>> {
        
           //todo yeah this is dumb
        let info_hash = file_handler::InfoHash::server_to_client_hash(file_hash.clone());
        let hash = info_hash.get_hashed_info_hash();

        for i in 0..num_connections {

            let request = Message::Request {
                index : 0,
                begin: file_hash.piece_length as u32 * i as u32,
                length: file_hash.piece_length as u32,
                hash,
            };

            
            println!("Sending piece request {}", i);
            self.request_txs.get(i % num_connections)
                .ok_or(Box::<dyn std::error::Error>::from("Could not retrieve connection rx"))?
                .send(request).await?;
            
        } 
        
        
        Ok(())
    }
    
     pub fn subscribe_new_connection(&mut self) -> mpsc::Receiver<Message> {
        let (request_tx, request_rx) = mpsc::channel::<Message>(50);
        self.request_txs.push(request_tx);
        
        request_rx
    }
    
    async fn reassemble_loop(mut conn_rx: mpsc::Receiver<Message>, info_hash: file_handler::InfoHash) 
        -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let num_pieces = info_hash.pieces.len();
        
        for _  in 0.. num_pieces {
           let msg = conn_rx.recv().await.ok_or("failed to get message")?;
           
           let (index, piece) = match msg {
               Message::Piece { index, piece  } => (index, piece),
               _ => Err(Box::<dyn std::error::Error + Send + Sync>::from("wrong message type"))?, 
           };

           println!("Received Piece: {}", index);
           write_piece_to_part(info_hash.clone(), piece, index)?;
           println!("Successfully Wrote: {}", index);
       }
        Ok(())
    }

}