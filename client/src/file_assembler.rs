use std::fs::File;
use std::sync::Arc;
use crate::connection::connection::*;
use crate::message::Message;
use crate::piece_assembler::*;
use tokio::sync::{mpsc, RwLock};
use crate::file_handler;
use crate::file_handler::write_piece_to_part;
use crate::torrent_client::TorrentClient;

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
    pub async fn assemble<'a>(
        torrent_client: &'a mut TorrentClient,
        mut peer_list: Vec<PeerId>,
        file_hash: InfoHash
    ) -> Result<(), Box<dyn std::error::Error>> {
        // let (snd_tx, snd_rx) = mpsc::channel::<Message>(10); // PieceAssembler sender/receiver
        let (conn_tx, conn_rx) = mpsc::channel::<Message>(50); // 'connection' sender/receiver

        
       let mut assembler = FileAssembler {
            // torrent_client,
            // peer_list,
            conn_tx,
            request_txs: Vec::new(),
        };

        //todo yeah this is dumb
        let info_hash = file_handler::InfoHash::server_to_client_hash(file_hash.clone());
        let hash = info_hash.get_hashed_info_hash();

        tokio::spawn(async move {
            let res = FileAssembler::reassemble_loop(conn_rx, info_hash).await;
            if res.is_err() {
                eprintln!("{:?}", res);
            }
        });

        //todo figure out how to delegate evenly to peers in list
        let peer = peer_list.pop().unwrap();

        
        let mut index = 0;
        let conn_tx = assembler.conn_tx.clone();
        let conn_rx = assembler.subscribe_new_connection();
        torrent_client.requester_connection(peer, conn_tx, conn_rx).await?;
        
        //todo this must be fixed with iterating through peerlist too
        let request_tx = assembler.request_txs.pop().unwrap();

        for piece in file_hash.pieces {

            let request = Message::Request {
                index,
                begin: file_hash.piece_length as u32 * index,
                length: file_hash.piece_length as u32,
                hash,
            };

            
            println!("Sending piece request {}", index);
            request_tx.send(request).await?;
            
            index += 1;
        } 
        
        
        Ok(())
    }
    
     fn subscribe_new_connection(&mut self) -> mpsc::Receiver<Message> {
        let (request_tx, request_rx) = mpsc::channel::<Message>(50);
        self.request_txs.push(request_tx);
        
        request_rx
    }
    
    async fn reassemble_loop(mut conn_rx: mpsc::Receiver<Message>, info_hash: file_handler::InfoHash) -> Result<(), Box<dyn std::error::Error>> {
        let num_pieces = info_hash.pieces.len();
        
        for _  in 0.. num_pieces {
           let msg = conn_rx.recv().await.ok_or("failed to get message")?;
           
           let (index, piece) = match msg {
               Message::Piece { index, piece  } => (index, piece),
               _ => Err(Box::<dyn std::error::Error>::from("wrong message type"))?, 
           };

           println!("Received Piece: {}", index);
           write_piece_to_part(info_hash.clone(), piece, index)?;
           println!("Successfully Wrote: {}", index);
       }
        Ok(())
    }

}