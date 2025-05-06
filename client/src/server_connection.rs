use tonic::transport::{Channel, ClientTlsConfig};
use crate::connection::connection::{connector_client, turn_client, ClientId, PeerId};

#[derive(Debug, Clone)]
pub struct ServerConnection {
    pub(crate) client: connector_client::ConnectorClient<Channel>,
    pub(crate) turn: turn_client::TurnClient<Channel>,
    pub(crate) uid: Option<ClientId>,
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

        let client = connector_client::ConnectorClient::new(endpoint.clone());
        let turn = turn_client::TurnClient::new(endpoint);

        Ok(
            ServerConnection {
                client,
                turn,
                uid: None,
            }
        )
    }

    pub async fn register_server_connection(&mut self, self_addr: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        let uid = self.client.register_client(self_addr).await?;
        self.uid = Some(uid.into_inner());

        Ok(())
    }
}
