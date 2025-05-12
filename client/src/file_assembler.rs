use std::sync::Arc;
use crate::connection::connection::{InfoHash};
use crate::message::Message;
use tokio::sync::{mpsc, RwLock};
use crate::{file_handler};
use crate::file_handler::write_piece_to_part;

/// this represents a connection between 2 peers
#[derive(Debug)]
pub struct FileAssembler {
    ///the sender used for LAN/P2P/QUIC to send data from
    conn_tx: mpsc::Sender<Message>, 
    /// sender used to send file requests across a connection
    request_txs: Arc<RwLock<Vec<mpsc::Sender<Message>>>>,
}

impl FileAssembler {

    pub async fn new(file_handler: InfoHash) -> Self {
        let (conn_tx, conn_rx) = mpsc::channel::<Message>(50);
        let assembler = FileAssembler {
            conn_tx,
            request_txs: Arc::new(RwLock::new(Vec::new())),
        };
        
        let request_txs = assembler.request_txs.clone();
        
        tokio::spawn(async move {
            let res = FileAssembler::reassemble_loop(conn_rx, file_handler, request_txs).await;
            if res.is_err() {
                eprintln!("{:?}", res);
            }
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

        let hash = file_hash.get_hashed_info_hash();
        
        for i in 0..file_hash.pieces.len() {

            let request = Message::Request {
                index : i as u32,
                begin: file_hash.piece_length as u32 * i as u32,
                length: file_hash.piece_length as u32,
                hash,
            };

            
            println!("Sending piece request {}", i);
            self.request_txs.read().await.get(i % num_connections)
                .ok_or(Box::<dyn std::error::Error>::from("Could not retrieve connection rx"))?
                .send(request).await?;

        }
        
        Ok(())
    }
    
     pub async fn subscribe_new_connection(&mut self) -> mpsc::Receiver<Message> {
        let (request_tx, request_rx) = mpsc::channel::<Message>(50);
        self.request_txs.write().await.push(request_tx);
        
        request_rx
    }
    
    async fn reassemble_loop(
        mut conn_rx: mpsc::Receiver<Message>, 
        info_hash: InfoHash, 
        request_txs: Arc<RwLock<Vec<mpsc::Sender<Message>>>>
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        
        loop {
           let msg = conn_rx.recv().await.ok_or("failed to get message")?;
           
           let (index, piece) = match msg {
               Message::Piece { index, piece  } => (index, piece),
               _ => Err(Box::<dyn std::error::Error + Send + Sync>::from("wrong message type"))?,
           };

           println!("Received Piece: {}", index);
           write_piece_to_part(info_hash.clone(), piece, index)?;
           println!("Successfully Wrote: {}", index);
            
            if file_handler::is_file_complete(info_hash.clone()) {
                break;
            }
        }
        
        for request_tx in request_txs.write().await.drain(0..) {
            drop(request_tx);
        }
        
        file_handler::build_file(info_hash)
            .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
        println!("piece built");
        
        Ok(())
    }

}