use std::fs::File;
use crate::connection::connection::*;
use crate::message::Message;
use crate::piece_assembler::*;
use tokio::sync::mpsc;


/// this represents a connection between 2 peers
#[derive(Debug)]
pub struct FileAssembler {
    /// the sender we will use to send to a PieceAssembler
    // snd_tx: mpsc::Sender<Message>,
    /// the receiver we will let a PieceAssembler borrow so we can send to it
    // snd_rx: mpsc::Receiver<Message>,
    
    /// the receiver we will get data from via LAN/P2P/QUIC
    conn_rx: mpsc::Receiver<Vec<u8>>, 
    
    ///the sender used for LAN/P2P/QUIC to send data from
    conn_tx: mpsc::Sender<Vec<u8>>, 
    /// sender used to send file requests across a connection
    request_txs: Vec<mpsc::Sender<Message>>,
}

impl FileAssembler {
    pub fn new() -> Self {
        // let (snd_tx, snd_rx) = mpsc::channel::<Message>(10); // PieceAssembler sender/receiver
        let (conn_tx, conn_rx) = mpsc::channel::<Vec<u8>>(10); // 'connection' sender/receiver

        
       FileAssembler {
            conn_rx,
            conn_tx,
            request_txs: Vec::new(),
        }
    }
    
    pub (crate) fn get_connection_send_handle(&self) -> mpsc::Sender<Vec<u8>> {
        self.conn_tx.clone()
    }
    
    pub (crate) fn subscribe_new_connection(&mut self) -> mpsc::Receiver<Message> {
        let (request_tx, request_rx) = mpsc::channel::<Message>(10);
        self.request_txs.push(request_tx);
        
        request_rx
    }

    async fn reassemble_loop(&mut self) {
       
    }

    // /// creates a new PieceAssembler and awaits the completed piece
    // pub async fn assemble_new_piece(mut rx: mpsc::Receiver<Message>) -> Vec<u8> {
    //     // todo fill in dummy values (0, 0)
    //     PieceAssembler::new(0, 0).assemble(&mut rx).await
    // }
}