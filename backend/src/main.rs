use axum::http::StatusCode;
use axum::{
    Router,
    body::Body,
    http::{Response, header},
    routing::get,
};
use std::env;
use std::io::{Write, stdout};
use std::net::SocketAddr;
use tokio::runtime::Builder;
use tower_http::services::ServeDir;

fn main() {
    let rt = Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(32_768)
        .build()
        .expect("Failed to create Tokio runtime");

    rt.block_on(async {
        let port = env::var("PORT").unwrap_or_else(|_| "8000".to_string());
        let addr: SocketAddr = format!("0.0.0.0:{port}").parse().expect("Invalid socket address");

        let app = Router::new()
            //- index
            .route("/", get(|| serve_static("static/index.html", &mime::TEXT_HTML)))
            .route("/index", get(|| serve_static("static/index.html", &mime::TEXT_HTML)))
            .route(
                "/index.html",
                get(|| serve_static("static/index.html", &mime::TEXT_HTML)),
            )
            .route(
                "/script.js",
                get(|| serve_static("static/script.js", &mime::APPLICATION_JAVASCRIPT)),
            )
            .route("/style.css", get(|| serve_static("static/style.css", &mime::TEXT_CSS)))
            //- contact
            .route(
                "/contact",
                get(|| serve_static("static/contact.html", &mime::TEXT_HTML)),
            )
            .route(
                "/contact.html",
                get(|| serve_static("static/contact.html", &mime::TEXT_HTML)),
            )
            .route(
                "/contact.js",
                get(|| serve_static("static/contact.js", &mime::APPLICATION_JAVASCRIPT)),
            )
            .route(
                "/contact.css",
                get(|| serve_static("static/contact.css", &mime::TEXT_CSS)),
            )
            //- projects
            .route(
                "/projects",
                get(|| serve_static("static/projects.html", &mime::TEXT_HTML)),
            )
            .route(
                "/projects.html",
                get(|| serve_static("static/projects.html", &mime::TEXT_HTML)),
            )
            .route(
                "/projects.js",
                get(|| serve_static("static/projects.js", &mime::APPLICATION_JAVASCRIPT)),
            )
            .route(
                "/projects.css",
                get(|| serve_static("static/projects.css", &mime::TEXT_CSS)),
            )
            //-static assets
            .nest_service("/static/assets", ServeDir::new("/static/assets"))
            //-commonjs
            .route(
                "/common.js",
                get(|| serve_static("static/common.js", &mime::APPLICATION_JAVASCRIPT)),
            );

        println!("listening on {addr}");
        stdout().flush().ok();

        let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");

        axum::serve(listener, app).await.expect("Server error");
    });
}

#[inline(always)]
async fn serve_static(path: &str, mime: &mime::Mime) -> Response<Body> {
    let (status, content) = match tokio::fs::read_to_string(path).await {
        Ok(c) => (StatusCode::OK, c),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "<h1>Internal Server Error</h1>".into(),
        ),
    };
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(content))
        .unwrap()
}
