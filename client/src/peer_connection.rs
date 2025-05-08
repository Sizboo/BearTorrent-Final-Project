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
    rcv_rx: mpsc::Receiver<Vec<u8>>, //potentially just receive raw bytes from pieceA
   
    /// sender used to send file requests across a connection
    request_tx: mpsc::Sender<Message>,
}

impl FileAssembler {
    pub fn new() -> (mpsc::Sender<Vec<u8>>, mpsc::Receiver<Message>) {
        // let (snd_tx, snd_rx) = mpsc::channel::<Message>(10); // PieceAssembler sender/receiver
        let (rcv_tx, rcv_rx) = mpsc::channel::<Vec<u8>>(10); // 'connection' sender/receiver
        let (request_tx, request_rx) = mpsc::channel::<Message>(10); 

        let mut conn = FileAssembler { rcv_rx, request_tx };
        
        

        tokio::spawn(async move {
            conn.receive_loop().await;
        });

        (rcv_tx, request_rx)
    }

    async fn receive_loop(&mut self) {
        while let Some(msg) = self.rcv_rx.recv().await {

            // if let Message::Block { block, .. } = msg {
            // 
            // }

            // todo parse flags and start piece assembly
        }
    }

    // /// creates a new PieceAssembler and awaits the completed piece
    // pub async fn assemble_new_piece(mut rx: mpsc::Receiver<Message>) -> Vec<u8> {
    //     // todo fill in dummy values (0, 0)
    //     PieceAssembler::new(0, 0).assemble(&mut rx).await
    // }
}