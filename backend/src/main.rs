use axum::http::StatusCode;
use axum::{Router, response::Html, routing::get};
use std::env;
use std::io::{Write, stdout};
use std::net::SocketAddr;
use tokio::runtime::Builder;

fn main() {
    let rt = Builder::new_multi_thread()
        .enable_all() // Enables I/O, time, etc.
        .max_blocking_threads(2048)
        .build()
        .expect("Failed to create Tokio runtime");

    // 2. Block on the async main logic
    rt.block_on(async {
        let port = env::var("PORT").unwrap_or_else(|_| "8000".to_string());
        let addr: SocketAddr = format!("0.0.0.0:{port}").parse().expect("Invalid socket address");

        let app = Router::new().route("/", get(get_index_html));

        println!("listening on {addr}");
        stdout().flush().ok();

        let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");

        axum::serve(listener, app).await.expect("Server error");
    });
}

async fn get_index_html() -> (StatusCode, Html<String>) {
    match tokio::fs::read_to_string("static/index.html").await {
        Ok(contents) => (StatusCode::OK, Html(contents)),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("<h1>Internal Server Error</h1>".to_string()),
        ),
    }
}
