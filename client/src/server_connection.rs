use tonic::transport::{Channel, ClientTlsConfig};
use crate::connection::connection::{connector_client, turn_client, ClientId, ClientRegistry, PeerId};

#[derive(Debug, Clone)]
pub struct ServerConnection {
    pub(crate) client: connector_client::ConnectorClient<Channel>,
    pub(crate) turn: turn_client::TurnClient<Channel>,
    pub(crate) uid: ClientId,
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

        Ok(
            ServerConnection {
                client,
                turn,
                uid,
            }
        )
    }

    pub async fn update_registered_peer_id(&mut self, self_addr: PeerId) -> Result<(), Box<dyn std::error::Error>> {

        Ok(())
    }
}
