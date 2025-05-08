use std::collections::HashMap;
use tonic::transport::{Channel, ClientTlsConfig};
use crate::connection::connection::{connector_client, turn_client, ClientId, ClientRegistry, FullId, PeerId};
use crate::file_handler::{get_info_hashes, InfoHash};

#[derive(Debug, Clone)]
pub struct ServerConnection {
    pub(crate) client: connector_client::ConnectorClient<Channel>,
    pub(crate) turn: turn_client::TurnClient<Channel>,
    pub(crate) uid: ClientId,
    pub(crate) file_hashes: HashMap<Vec<u8>, InfoHash>
}

const GCLOUD_URL: &str = "https://helpful-serf-server-1016068426296.us-south1.run.app:";

impl ServerConnection {
    pub (crate) async fn new() -> Result<ServerConnection, Box<dyn std::error::Error>> {
        //tls config
        //webki roots uses Mozilla's certificate store
        let tls = ClientTlsConfig::new()
            .with_webpki_roots()
            .domain_name("helpful-serf-server-1016068426296.us-south1.run.app");

        let endpoint = Channel::from_static(GCLOUD_URL).tls_config(tls)?
            .connect().await?;

        let mut client = connector_client::ConnectorClient::new(endpoint.clone());
        let turn = turn_client::TurnClient::new(endpoint);
        
        let uid = client.register_client(ClientRegistry { peer_id: None} ).await?;
        let uid = uid.into_inner();

        let file_hashes = match get_info_hashes(){
            Ok(file_hashes) => file_hashes,
            Err(err) => return Err(Box::new(err)),
        };

        Ok(
            ServerConnection {
                client,
                turn,
                uid,
                file_hashes,
            }
        )
    }

    pub async fn update_registered_peer_id(&mut self, self_addr: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        
        let mut server_conn = self.client.clone();
        
        
        match server_conn.update_registered_peer_id(
            FullId { 
                self_id: Option::from(self.uid.clone()), 
                peer_id: Some(self_addr)
            }
        ).await {
            Ok(res) => Ok(()),
            Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
        }
        
    }
}
