use crate::connection::connection::*;
use tokio::sync::mpsc;

// TODO decide how we want to receive the data... piece by piece? if so, include piece hash?
#[derive(Debug, Clone)]
pub struct SocketData {
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct DataRouter {
    rx: mpsc::Receiver<SocketData>,
}

impl DataRouter {
    pub fn new() -> (Self, mpsc::Sender<SocketData>) {
        let (tx, rx) = mpsc::channel::<SocketData>(10);
        ( DataRouter { rx }, tx )
    }

    pub async fn run(
        &mut self
    ) {
        println!("Processing data...");
        while let Some(data) = self.rx.recv().await {
            println!("{:?}", data.data);
        }
    }
}