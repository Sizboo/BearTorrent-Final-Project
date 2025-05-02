use crate::connection::connection::*;
use tokio::sync::mpsc;

// TODO decide how we want to receive the data... piece by piece? if so, include piece hash?
pub struct SocketData {

}

pub struct DataHandler {
    rx: mpsc::Receiver<SocketData>,
}

impl DataHandler {}