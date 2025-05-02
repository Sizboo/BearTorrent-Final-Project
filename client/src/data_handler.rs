use crate::connection::connection::*;
use tokio::sync::mpsc;

// TODO decide how we want to receive the data... piece by piece? if so, include piece hash?
#[derive(Debug, Clone)]
pub struct SocketData {
    data: Bytes,
}

#[derive(Debug, Clone)]
pub struct DataHandler {
    rx: mpsc::Receiver<SocketData>,
}

impl DataHandler {
}