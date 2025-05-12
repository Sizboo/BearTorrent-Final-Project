use crate::connection::connection::{InfoHash};
use crate::message::Message;
use tokio::sync::{mpsc};
use crate::{file_handler};
use crate::file_handler::write_piece_to_part;

/// this represents a connection between 2 peers
#[derive(Debug)]
pub struct FileAssembler {

    ///the sender used for LAN/P2P/QUIC to send data from
    conn_tx: mpsc::Sender<Message>, 
    /// sender used to send file requests across a connection
    request_txs: Vec<mpsc::Sender<Message>>,
}

impl FileAssembler {

    pub async fn new(file_handler: InfoHash) -> Self {
        let (conn_tx, conn_rx) = mpsc::channel::<Message>(50);
        let assembler = FileAssembler {
            conn_tx,
            request_txs: Vec::new(),
        };

        tokio::spawn(async move {
            let res = Self::reassemble_loop(conn_rx, file_handler).await;
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
    ) -> Result<(), String> {
        let hash = file_hash.get_hashed_info_hash();

        for i in 0..file_hash.pieces.len() {
            let request = Message::Request {
                index: i as u32,
                begin: file_hash.piece_length as u32 * i as u32,
                length: file_hash.piece_length as u32,
                hash,
            };

            println!("Sending piece request {}", i);
            let sender = self.request_txs.get(i % num_connections)
                .ok_or_else(|| format!("No connection sender found for index {}", i))?;
            sender.send(request).await
                .map_err(|e| format!("Send failed: {}", e))?;
        }

        Ok(())
    }


    pub fn subscribe_new_connection(&mut self) -> mpsc::Receiver<Message> {
        let (request_tx, request_rx) = mpsc::channel::<Message>(50);
        self.request_txs.push(request_tx);
        
        request_rx
    }

    async fn reassemble_loop(
        mut conn_rx: mpsc::Receiver<Message>,
        info_hash: InfoHash
    ) -> Result<(), String> {
        let num_pieces = info_hash.pieces.len();

        for _ in 0..num_pieces {
            let msg = conn_rx.recv().await.ok_or("Failed to get message".to_string())?;

            let (index, piece) = match msg {
                Message::Piece { index, piece } => (index, piece),
                _ => return Err("Received non-piece message in reassembler".to_string()),
            };

            println!("Received Piece: {}", index);
            write_piece_to_part(info_hash.clone(), piece, index)
                .map_err(|e| format!("Failed to write piece: {}", e))?;
            println!("Successfully Wrote: {}", index);
        }

        file_handler::build_file(info_hash)
            .map_err(|e| format!("Failed to build file: {}", e))?;

        println!("File assembled");
        Ok(())
    }


}