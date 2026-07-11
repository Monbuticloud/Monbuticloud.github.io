mod games;

use axum::{
    Router,
    body::Body,
    extract::Query,
    http::{Response, StatusCode, header},
    middleware::{self, Next},
    routing::get,
};
use chrono::{DateTime, Utc};
use crossbeam::queue::SegQueue;
use std::{
    collections::HashMap,
    env,
    io::{Write, stdout},
    net::SocketAddr,
    sync::{LazyLock, RwLock},
    time::Duration,
};
use tokio::runtime::Builder;
use tower_http::services::ServeDir;

static REQUEST_LOG: LazyLock<SegQueue<(String, DateTime<Utc>)>> = LazyLock::new(SegQueue::new);

static FILE_CACHE: LazyLock<RwLock<HashMap<&'static str, (StatusCode, String, &'static mime::Mime)>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn main() {

    // ── Background flusher: drain queue to stdout every 500ms ──
    let ticker = crossbeam::channel::tick(Duration::from_millis(500));

    std::thread::spawn(move || {
        loop {

            ticker.recv().unwrap();

            let mut buf = String::new();

            while let Some((path, time)) = REQUEST_LOG.pop() {

                use std::fmt::Write;

                let _ = writeln!(buf, "[{}] {}", time.format("%H:%M:%S"), path);
            }

            if !buf.is_empty() {

                print!("{buf}");

                stdout().flush().ok();
            }
        }
    });

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
            .route("/", get(|| serve_static("static/index.html")))
            .route("/index", get(|| serve_static("static/index.html")))
            .route("/index.html", get(|| serve_static("static/index.html")))
            .route("/script.js", get(|| serve_static("static/script.js")))
            .route("/style.css", get(|| serve_static("static/style.css")))
            //- contact
            .route("/contact", get(|| serve_static("static/contact.html")))
            .route("/contact.html", get(|| serve_static("static/contact.html")))
            .route("/contact.js", get(|| serve_static("static/contact.js")))
            .route("/contact.css", get(|| serve_static("static/contact.css")))
            //- projects
            .route("/projects", get(|| serve_static("static/projects.html")))
            .route("/projects.html", get(|| serve_static("static/projects.html")))
            .route("/projects.js", get(|| serve_static("static/projects.js")))
            .route("/projects.css", get(|| serve_static("static/projects.css")))
            //-static assets
            .nest_service("/static/assets", ServeDir::new("/static/assets"))
            //-common.js
            .route("/common.js", get(|| serve_static("static/common.js")))
            //-common.css
            .route("/common.css", get(|| serve_static("static/common.css")))
            //- chess API
            .route("/api/games/chess/completions", get(get_chess_completion))
            //-404
            .layer(middleware::from_fn(track_request));

        println!("listening on {addr}");

        stdout().flush().ok();

        let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");

        axum::serve(listener, app).await.expect("Server error");
    });
}

fn mime_for_path(path: &str) -> &'static mime::Mime {

    if path.ends_with(".html") {

        &mime::TEXT_HTML
    } else if path.ends_with(".css") {

        &mime::TEXT_CSS
    } else if path.ends_with(".js") {

        &mime::APPLICATION_JAVASCRIPT
    } else {

        &mime::TEXT_PLAIN
    }
}

#[inline(always)]

async fn serve_static(path: &'static str) -> Response<Body> {

    if let Some(cached) = FILE_CACHE.read().unwrap().get(path) {

        let (status, content, mime) = cached;

        return Response::builder()
            .status(*status)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.clone()))
            .unwrap();
    }

    let mime = mime_for_path(path);

    let (status, content) = match tokio::fs::read_to_string(path).await {
        Ok(c) => (StatusCode::OK, c),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "<h1>Internal Server Error</h1>".into()),
    };

    FILE_CACHE
        .write()
        .unwrap()
        .insert(path, (status, content.clone(), mime));

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(content))
        .unwrap()
}

async fn track_request(request: axum::http::Request<Body>, next: Next) -> Response<Body> {

    let path = request.uri().path().to_owned();

    let time = Utc::now();

    REQUEST_LOG.push((path, time));

    next.run(request).await
}

// ── Chess API ──

fn json_body(status: StatusCode, body: String) -> Response<Body> {

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

/// GET /api/games/chess/completions?fen=...
///
/// Fixed depth=6 to avoid CPU-exhaustion via arbitrary depth params.
/// Runs the sunfish-inspired chess engine on a blocking thread
/// so the async runtime isn't starved.

async fn get_chess_completion(params: Query<HashMap<String, String>>) -> Response<Body> {

    let fen = match params.get("fen") {
        Some(f) => f,
        None => return json_body(StatusCode::BAD_REQUEST, r#"{"error":"missing 'fen' query parameter"}"#.into()),
    };

    let fen = fen.clone();

    let result = tokio::task::spawn_blocking(move || games::chess::best_move(&fen, 6)).await;

    match result {
        Ok(Some(best_move)) => json_body(StatusCode::OK, format!(r#"{{"best_move":"{best_move}"}}"#)),
        Ok(None) => json_body(StatusCode::BAD_REQUEST, r#"{"error":"no legal moves or invalid FEN"}"#.into()),
        Err(join_err) => {
            json_body(StatusCode::INTERNAL_SERVER_ERROR, format!(r#"{{"error":"search panicked: {join_err}"}}"#))
        },
    }
}
