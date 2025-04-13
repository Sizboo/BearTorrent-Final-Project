// mod build;
//
// use std::env;
// use warp::Filter;
//
// #[tokio::main]
// async fn main() {
//     let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
//     let routes = warp::any().map(|| warp::reply::html("Hello, world!"));
//
//     warp::serve(routes)
//         .run(([0, 0, 0, 0], port.parse().unwrap()))
//         .await;
//
// }

use std::env;
use tonic::{transport::Server, Request, Response, Status};
use connection::{ConnReq, connector_server::{Connector, ConnectorServer}};
pub mod connection {
    tonic::include_proto!("connection");
}

#[derive(Debug, Default)]
pub struct ConnectionService {}

#[tonic::async_trait]
impl Connector for ConnectionService {
    async fn send_data(&self, request: Request<ConnReq>) -> Result<Response<ConnReq>, Status> {
        let r = request.into_inner();
        println!("Sending data: {:?}", r);
        Ok(Response::new(connection::ConnReq {
            ipaddr: r.ipaddr,
            port: r.port,
            info: r.info,
        }))
    }
}



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let address = format!("0.0.0.0:{}", port).parse()?;
    let connection_service = ConnectionService::default();

    Server::builder()
        .add_service(ConnectorServer::new(connection_service))
        .serve(address)
        .await?;

    Ok(())
}