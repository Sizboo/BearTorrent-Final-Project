use tonic::transport::{Channel, ClientTlsConfig};
use crate::torrent_client::connection::{PeerId, connector_client::ConnectorClient, ClientId};

#[derive(Debug, Clone)]
pub struct ServerConnection {
    pub(crate) client: ConnectorClient<Channel>,
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

        let client = ConnectorClient::new(endpoint);

        //todo use this to figure out id persistence across sessions
        //1. get uuid
        // let dirs = directories_next::ProjectDirs::from("org", "helpful_serf", "torrent_client")
        //     .ok_or_else(|| Box::<dyn std::error::Error>::from("Could not get project dirs"))?;
        // let path = dirs.data_local_dir();
        // let uid_path = path.join("uuid.der");
        // 
        // let mut uid= "".to_string();
        // 
        // if uid_path.exists() {
        //     uid = std::fs::read_to_string(&uid_path)?;
        //     
        //     //todo verify server has uid
        // }
        // //2. get new uid from server 
        // else {
        //     
        // }
        //update id information
        // fs::create_dir_all(&path)?;
        // fs::write(&uid_path, uid.clone())?;

        Ok(
            ServerConnection {
                client,
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
