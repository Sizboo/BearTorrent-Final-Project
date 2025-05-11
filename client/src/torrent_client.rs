use std::cmp::min;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddrV4, ToSocketAddrs};
use std::sync::Arc;

use local_ip_address::local_ip;
use stunclient::StunClient;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tonic::Request;
use tonic::transport::{Channel, ClientTlsConfig};

use crate::connection::connection::{
    connector_client, turn_client, ClientId, ClientRegistry, FileMessage, FullId, InfoHash, PeerId,
};
use crate::file_assembler::FileAssembler;
use crate::file_handler::get_info_hashes;
use crate::peer_connection::PeerConnection;

#[derive(Debug, Clone)]
pub struct TorrentClient {
    pub(crate) client: connector_client::ConnectorClient<Channel>,
    pub(crate) turn: turn_client::TurnClient<Channel>,
    pub(crate) uid: ClientId,
    pub(crate) file_hashes: Arc<RwLock<HashMap<[u8; 20], InfoHash>>>,
}

const GCLOUD_URL: &str = "https://helpful-serf-server-1016068426296.us-south1.run.app:";

impl TorrentClient {
    pub async fn new() -> Result<TorrentClient, String> {
        let tls = ClientTlsConfig::new()
            .with_webpki_roots()
            .domain_name("helpful-serf-server-1016068426296.us-south1.run.app");

        let endpoint = Channel::from_static(GCLOUD_URL)
            .tls_config(tls)
            .map_err(|e| format!("TLS config failed: {}", e))?
            .connect()
            .await
            .map_err(|e| format!("Channel connect failed: {}", e))?;

        let mut client = connector_client::ConnectorClient::new(endpoint.clone());
        let turn = turn_client::TurnClient::new(endpoint);

        let uid = client
            .register_client(ClientRegistry { peer_id: None })
            .await
            .map_err(|e| format!("Register client failed: {}", e))?
            .into_inner();

        let file_hashes = get_info_hashes().map_err(|e| format!("File hash read error: {}", e))?;



        Ok(TorrentClient {
            client,
            turn,
            uid,
            file_hashes: Arc::new(RwLock::new(file_hashes)),
        })
    }

    async fn register_new_connection(&mut self) -> Result<PeerConnection, String> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("UDP bind failed: {}", e))?;

        let stun_server = "stun.l.google.com:19302"
            .to_socket_addrs()
            .map_err(|e| format!("STUN resolve failed: {}", e))?
            .find(|x| x.is_ipv4())
            .ok_or_else(|| "No IPv4 STUN server found".to_string())?;

        let client = StunClient::new(stun_server);
        let external_addr = client
            .query_external_address(&socket)
            .map_err(|e| format!("STUN query failed: {}", e))?;

        let ipaddr = match external_addr.ip() {
            IpAddr::V4(v4) => u32::from_be_bytes(v4.octets()),
            IpAddr::V6(_) => return Err("Cannot convert IPv6 to u32".into()),
        };

        let priv_ipaddr = match local_ip().map_err(|e| format!("Get local IP failed: {}", e))? {
            IpAddr::V4(v4) => v4,
            IpAddr::V6(_) => return Err("Cannot convert local IPv6 to u32".into()),
        };

        let priv_addr = SocketAddrV4::new(priv_ipaddr, 0);
        let priv_socket = UdpSocket::bind(priv_addr)
            .await
            .map_err(|e| format!("Private socket bind failed: {}", e))?;

        let priv_port = priv_socket
            .local_addr()
            .map_err(|e| format!("Get local port failed: {}", e))?
            .port();

        socket
            .set_nonblocking(true)
            .map_err(|e| format!("Set non-blocking failed: {}", e))?;

        let self_addr = PeerId {
            ipaddr,
            port: external_addr.port() as u32,
            priv_ipaddr: u32::from_be_bytes(priv_ipaddr.octets()),
            priv_port: priv_port as u32,
        };

        self.update_registered_peer_id(self_addr.clone()).await?;

        let server = self.clone();

        Ok(PeerConnection {
            server,
            pub_socket: Some(UdpSocket::try_from(socket).map_err(|e| format!("TryFrom failed: {}", e))?),
            priv_socket: Some(priv_socket),
            self_addr,
        })
    }

    async fn update_registered_peer_id(&mut self, self_addr: PeerId) -> Result<(), String> {
        let mut server_conn = self.client.clone();

        server_conn
            .update_registered_peer_id(FullId {
                self_id: Some(self.uid.clone()),
                peer_id: Some(self_addr),
            })
            .await
            .map_err(|e| format!("Update registered peer ID failed: {}", e))?;

        Ok(())
    }

    pub async fn seeding(&mut self) -> Result<(), String> {
        loop {
            let mut peer_connection = self.register_new_connection().await?;
            let mut server_client = self.client.clone();

            self.advertise_all().await?;

            let response = server_client
                .seed(peer_connection.self_addr.clone())
                .await;

            match response {
                Ok(res) => {
                    tokio::spawn(async move {
                        if let Err(e) = peer_connection.seeder_connection(res).await {
                            eprintln!("Connect Failed: {e}");
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Failed to get peer: {e}");
                }
            }
        }
    }

    pub async fn file_request(&mut self, file_hash: InfoHash) -> Result<(), String> {
        let mut client = self.client.clone();

        let peer_list = client
            .get_file_peer_list(file_hash.clone())
            .await
            .map_err(|e| format!("Get peer list failed: {}", e))?
            .into_inner()
            .list;

        let num_connections = min(peer_list.len(), file_hash.pieces.len());
        let mut assembler = FileAssembler::new(file_hash.clone()).await;


        let mut connection_handles = Vec::new();

        for i in 0..num_connections {
            let mut peer_connection = self.register_new_connection().await?;
            let conn_tx = assembler.get_conn_tx();
            let request_rx = assembler.subscribe_new_connection();
            let peer_id = peer_list[i];

            let handle = tokio::spawn(async move {
                if let Err(e) = peer_connection.requester_connection(peer_id, conn_tx, request_rx).await {
                    eprintln!("Connection error: {e}");
                }
            });

            connection_handles.push(handle);
        }

        assembler
            .send_requests(num_connections, file_hash)
            .await
            .map_err(|e| format!("Send requests failed: {}", e))?;


        for handle in connection_handles {
            handle.await.map_err(|e| format!("Task join error: {e}"))?;
        }

        Ok(())
    }

    pub async fn advertise(&self, info_hash: InfoHash) -> Result<ClientId, String> {
        let mut client = self.client.clone();

        let request = Request::new(FileMessage {
            id: Some(self.uid.clone()),
            info_hash: Some(info_hash),
        });

        let resp = client
            .advertise(request)
            .await
            .map_err(|e| format!("Advertise failed: {}", e))?;

        Ok(resp.into_inner())
    }

    async fn advertise_all(&self) -> Result<(), String> {
        let my_hashes = self.file_hashes.read().await.clone().into_values();
        for hash in my_hashes {
            self.advertise(hash).await?;
        }
        Ok(())
    }

    pub async fn get_server_files(&self) -> Result<Vec<InfoHash>, String> {
        let mut server_connection = self.client.clone();

        let res = server_connection
            .get_all_files(Request::new(()))
            .await
            .map_err(|e| format!("gRPC request failed: {}", e))?;

        Ok(res.into_inner().info_hashes)
    }

    pub async fn announce(&self) -> Result<(), String> {
        Ok(())
    }
}
