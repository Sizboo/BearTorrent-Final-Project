use std::sync::Arc;
use crate::connection::connection::{InfoHash};
use crate::message::Message;
use tokio::sync::{mpsc, Notify, RwLock};
use crate::{file_handler};
use crate::file_handler::{hash_piece_data, write_piece_to_part};

/// this represents a connection between 2 peers
#[derive(Debug)]
pub struct FileAssembler {
    ///file hash that is being requested
    file_hash: InfoHash,
    ///notify handle used to tell send_requests to begin requesting
    start_requesting: Arc<Notify>,
    ///number of active peer connections achieved
    num_connections: usize,
    ///the sender used for LAN/P2P/QUIC to send data from
    conn_tx: mpsc::Sender<Message>,
    /// sender used to send file requests across a connection
    request_txs: Vec<mpsc::Sender<Message>>,
}

impl FileAssembler {

    ///FileAssembler::new() 
    /// parameters:
    ///     - file_hash: the InfoHash object of the file requesting
    ///     - num_connections: the number of successful p2p connections
    /// 
    /// function:
    /// This method creates a new FileAssembler object within Arc<RwLock<>>.
    /// It spawns off both necessary file assembly processes, one for sending requests
    /// and one for reassembling a file from pieces.
    pub async fn new(file_hash: InfoHash, num_connection: usize) -> Arc<RwLock<FileAssembler>> {
        let (conn_tx, conn_rx) = mpsc::channel::<Message>(150);
        let assembler = FileAssembler {
            file_hash: file_hash.clone(),
            start_requesting: Arc::new(Notify::new()),
            num_connections: num_connection,
            conn_tx,
            request_txs: Vec::new(),
        };

        let (resend_tx, resend_rx) = mpsc::channel::<Message>(100);

        let assembler = Arc::new(RwLock::new(assembler));

        let assembler_clone = assembler.clone();
        tokio::spawn(async move {
            let res = FileAssembler::reassemble_loop(conn_rx, assembler_clone, resend_tx).await;
            if res.is_err() {
                eprintln!("Reassembly Loop Error: {:?}", res);
            }
        });

        let assembler_clone = assembler.clone();
        let hash = file_hash.get_hashed_info_hash();
        tokio::spawn(async move {
            let res = FileAssembler::send_requests(hash, assembler_clone, resend_rx).await;
            if res.is_err() {
                eprintln!("Send Request Error: {:?}", res);
            }
        });

        assembler
    }

    ///get_conn_tx()
    /// parameters:
    ///     - self: to copy conn_tx
    /// 
    ///function:
    ///Returns a clone of internal conn_tx which connections will 
    ///use to communicate to the reassemble process.
    pub fn get_conn_tx(&self) -> mpsc::Sender<Message> {
        self.conn_tx.clone()
    }
    
    ///subscribe_new_connection()
    ///parameters:
    ///    - mut self: self to add tx to array
    ///
    ///function:
    ///This method "subscribes" a new connection by adding a tx to an internal
    ///vector of tx handles and returning the associated receiver
    ///so that the send_requests method can send requests to connections. 
    pub fn subscribe_new_connection(&mut self) -> mpsc::Receiver<Message> {
        let (request_tx, request_rx) = mpsc::channel::<Message>(150);
        self.request_txs.push(request_tx);

        request_rx
    }

    
    /// start_requesting begins the requesting process
    /// this should only be called once connections have been
    /// successfully established. 
    pub fn start_requesting(&mut self) {
        println!("Notifying waiters");
        self.start_requesting.notify_waiters();
    }

    ///send_requests
    /// parameters:
    ///     - hash: this is the 20 byte hash of the InfoHash for the requested file
    ///     - assembler: this is a reference to the shared assembler object
    ///     - resend_rx: receiving end of channel used to get resend requests from reassemble loop
    /// 
    /// function:
    /// This method loops through all connections evenly splitting the number of file
    /// pieces amongst connected peers. Once it sends out all initial requests, it listens
    /// for resend requests which will be issued by reassemble_loop. Once reassemble_loop
    /// gets the complete file, it will drop the resend_tx ending this process.
    async fn send_requests(
        hash: [u8; 20],
        assembler: Arc<RwLock<FileAssembler>>,
        mut resend_rx: mpsc::Receiver<Message>, //used to resend requests for pieces that didn't come or are incorrect
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

        //get necessary fields in an efficient manner
        let assembler_lock = assembler.read().await;
        let num_pieces = assembler_lock.file_hash.pieces.len();
        let num_connections = assembler_lock.num_connections;
        let piece_length = assembler_lock.file_hash.piece_length;
        drop(assembler_lock);

        
        //wait for connections to have been established to start requesting
        let notify_handle = assembler.read().await.start_requesting.clone();
        notify_handle.notified().await;

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

                println!("Sending request");

                let new_seeder = (i % assembler.read().await.num_connections) as u32;

                //make a new request message with the new seeder
                let msg = match msg {
                    Message::Cancel {index, begin, length, .. } =>
                        Message::Request {seeder: new_seeder, index, begin, length, hash},
                    Message::Piece { index, ..} =>
                        Message::Request {
                            seeder: new_seeder,
                            index,
                            begin: piece_length * index,
                            length: piece_length,
                            hash},
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
    

    ///reassemble_loop
    /// parameters:
    ///    - conn_rx: receiving end to get piece messages back from connections
    ///    - assembler: this is a reference to the shared assembler object
    ///    - resend_tx: sending end of resend channel to send resend requests to peer
    /// 
    /// function:
    /// This method waits until a file is completed or it fails to retrieve a file from underlying
    /// connections. It takes each piece, checking it is valid with hash and constructs the file. 
    /// If the hash does not line up, or the underlying connection fails, sending a cancel request
    /// it will transmit a resend request to send_requests. 
    async fn reassemble_loop(
        mut conn_rx: mpsc::Receiver<Message>, //used to receive messages back from connection
        assembler: Arc<RwLock<FileAssembler>>,
        resend_tx: mpsc::Sender<Message>, //used to send resend requests to send_requests loop
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

        let info_hash = assembler.read().await.file_hash.clone();
        let piece_length = assembler.read().await.file_hash.piece_length;


        loop {
           let msg = conn_rx.recv().await.ok_or("failed to get message")?;
           
           match msg {
               Message::Piece { index,  piece } => {
                   println!("Received Piece: {}", index);

                   //We want to verify the piece was not corrupted across transport.
                   //If it was, we want to resend a request for a new piece.
                   if  piece.len() == piece_length as usize {
                       let recvd_piece_hash = hash_piece_data(piece.clone());

                       let expected_hash: [u8; 20] = info_hash.pieces
                           .get(index as usize).cloned()
                           .ok_or(Box::<dyn std::error::Error + Send + Sync>::from("Could not retrieve piece hash"))?
                           .hash.try_into().map_err(|_| Box::<dyn std::error::Error + Send + Sync>::from("Could not convert piece hash"))?;

                       if recvd_piece_hash != expected_hash {
                           println!("Piece corrupted sending resend request");
                           resend_tx.send(Message::Piece { index, piece }).await?;
                           continue;
                       }
                   }

                   write_piece_to_part(info_hash.clone(), piece, index)?;
                   println!("Successfully Wrote: {}", index);

                   if file_handler::is_file_complete(info_hash.clone()) {
                       println!("File complete!");
                       break;
                   }
               },
               Message::Cancel {seeder,index, begin, length} => {
                   println!("Failed to get piece removing seeder and trying again");

                   //if we get a cancel notification, we are going to assume this means the seeder
                   //does not or cannot provide the data. So we will remove it from seeder list
                   //and resend a request.

                   let bad_tx = assembler.write().await.request_txs.remove(seeder as usize);
                   assembler.write().await.num_connections -= 1;
                   drop(bad_tx);

                   if assembler.read().await.num_connections == 0 {
                       drop(resend_tx);
                       return Err(Box::<dyn std::error::Error + Send + Sync>::from("Failed to Retrieve File"))
                   }

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