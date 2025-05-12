use std::sync::Arc;
use crate::connection::connection::{InfoHash};
use crate::message::Message;
use tokio::sync::{mpsc, Notify, RwLock};
use crate::{file_handler};
use crate::file_handler::write_piece_to_part;

/// this represents a connection between 2 peers
#[derive(Debug)]
pub struct FileAssembler {
    ///file hash that is being requested
    file_hash: InfoHash,
    ///notify handle used to tell send_requests to begin requesting
    start_requesting: Notify,
    ///number of active peer connections achieved
    num_connections: usize,
    ///the sender used for LAN/P2P/QUIC to send data from
    conn_tx: mpsc::Sender<Message>,
    /// sender used to send file requests across a connection
    request_txs: Vec<mpsc::Sender<Message>>,
}

impl FileAssembler {

    pub async fn new(file_hash: InfoHash, num_connection: usize) -> Arc<RwLock<FileAssembler>> {
        let (conn_tx, conn_rx) = mpsc::channel::<Message>(150);
        let assembler = FileAssembler {
            file_hash: file_hash.clone(),
            start_requesting: Notify::new(),
            num_connections: num_connection,
            conn_tx,
            request_txs: Vec::new(),
        };

        let (resend_tx, resend_rx) = mpsc::channel::<Message>(10);

        // let request_txs = assembler.request_txs.clone();

        let assembler = Arc::new(RwLock::new(assembler));

        let assembler_clone = assembler.clone();
        tokio::spawn(async move {
            let res = FileAssembler::reassemble_loop(conn_rx, assembler_clone, resend_tx).await;
            if res.is_err() {
                eprintln!("Reassembly Loop Error: {:?}", res);
            }
        });

        let assembler_clone = assembler.clone();
        tokio::spawn(async move {
            let res = FileAssembler::send_requests(assembler_clone, resend_rx).await;
            if res.is_err() {
                eprintln!("Send Request Error: {:?}", res);
            }
        });

        assembler
    }

    pub fn get_conn_tx(&self) -> mpsc::Sender<Message> {
        self.conn_tx.clone()
    }

    pub fn subscribe_new_connection(&mut self) -> mpsc::Receiver<Message> {
        let (request_tx, request_rx) = mpsc::channel::<Message>(150);
        self.request_txs.push(request_tx);

        request_rx
    }

    pub fn start_requesting(&mut self) {
        println!("Notifying waiters");
        self.start_requesting.notify_waiters();
    }

    async fn send_requests(
        assembler: Arc<RwLock<FileAssembler>>,
        mut resend_rx: mpsc::Receiver<Message>, //used to resend requests for pieces that didn't come or are incorrect
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

        //get necessary fields in an efficient manner
        let assembler_lock = assembler.read().await;
        let hash = assembler_lock.file_hash.get_hashed_info_hash();
        let num_pieces = assembler_lock.file_hash.pieces.len();
        let num_connections = assembler_lock.num_connections;
        let piece_length = assembler_lock.file_hash.piece_length;
        drop(assembler_lock);

        println!("waiting for notify handle");
        
        //wait for connections to have been established to start requesting
        assembler.read().await.start_requesting.notified().await;

        //send initial requests
        for i in 0..num_pieces {

            let request = Message::Request {
                seeder: (i % num_connections) as u32,
                index : i as u32,
                begin: piece_length * i as u32,
                length: piece_length,
                hash,
            };

            println!("Sending piece request {}", i);
            assembler.read().await.request_txs.get(i % num_connections)
                .ok_or(Box::<dyn std::error::Error + Send + Sync>::from("Could not retrieve connection rx"))?
                .send(request).await?;

        }

        let mut i = 0;
        loop {
            if let Some(msg) = resend_rx.recv().await {

                println!("Sending request {:?}", msg);

                let new_seeder = (i % assembler.read().await.num_connections) as u32;

                //make a new request message with the new seeder
                let msg = match msg {
                    Message::Cancel {index, begin, length, .. } =>
                        Message::Request {seeder: new_seeder, index, begin, length, hash},
                    Message::Request { index, begin, length, hash, ..} =>
                        Message::Request {seeder: new_seeder, index, begin, length, hash},
                    _ => continue,
                };

                assembler.read().await.request_txs.get(new_seeder as usize)
                    .ok_or(Box::<dyn std::error::Error + Send + Sync>::from("Could not retrieve connection rx"))?
                    .send(msg).await?;

                i += 1;

            } else {
                println!("Sending Loop finishing!");
                return Ok(())
            }
        }
    }
    

    async fn reassemble_loop(
        mut conn_rx: mpsc::Receiver<Message>, //used to receive messages back from connection
        assembler: Arc<RwLock<FileAssembler>>,
        resend_tx: mpsc::Sender<Message>, //used to send resend requests to send_requests loop
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

        let info_hash = assembler.read().await.file_hash.clone();


        loop {
           let msg = conn_rx.recv().await.ok_or("failed to get message")?;
           
           match msg {
               Message::Piece { index, piece  } => {
                   println!("Received Piece: {}", index);
                   write_piece_to_part(info_hash.clone(), piece, index)?;
                   println!("Successfully Wrote: {}", index);

                   if file_handler::is_file_complete(info_hash.clone()) {
                       break;
                   }
               },
               Message::Cancel {seeder,index, begin, length} => {
                   println!("Failed to get piece removing seeder and trying again");

                   //if we get a cancel notification, we are going to assume this means the seeder
                   //does not or cannot provide the data. So we will remove it from seeder list
                   //and resend a request.

                   assembler.write().await.request_txs.remove(seeder as usize);
                   assembler.write().await.num_connections -= 1;

                   resend_tx.send(Message::Cancel {seeder, index, begin, length}).await?;

               },
               _ => Err(Box::<dyn std::error::Error + Send + Sync>::from("wrong message type"))?,
           };


        }

        //drop all senders signaling end of connection
        for request_tx in assembler.write().await.request_txs.drain(0..) {
            drop(request_tx);
        }
        drop(resend_tx);
        
        file_handler::build_file(info_hash)
            .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
        println!("piece built");
        
        Ok(())
    }

}