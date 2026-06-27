use axum::{response::Html, routing::get, Router};
use std::env;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let port = env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("Invalid socket address");

    let app = Router::new().route("/", get(handler));

    // eprintln goes to stderr which is unbuffered on musl
    eprintln!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello from Koyeb + Axum!</h1>")
}
