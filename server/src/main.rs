use std::env;
use warp::Filter;

#[tokio::main]
async fn main() {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let routes = warp::any().map(|| warp::reply::html("Hello, world!"));

    warp::serve(routes)
        .run(([0, 0, 0, 0], port.parse().unwrap()))
        .await;
}