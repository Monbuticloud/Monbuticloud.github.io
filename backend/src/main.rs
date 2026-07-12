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
    sync::{LazyLock, RwLock, atomic::{AtomicUsize, Ordering}},
    time::Duration,
};
use tokio::runtime::Builder;
use tower_http::services::ServeDir;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// Structured log messages — no freeform strings.
#[derive(Debug)]
pub(crate) enum LogMsg {
    Request { path: String },
    ChessSearch { tt_entries: usize, depth: usize, fen: String },
    ChessDepth { depth: usize, score: i32, best: String, is_valid: bool },
    ChessNoMove,
    ChessResult { best_move: String },
}

impl std::fmt::Display for LogMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogMsg::Request { path } => write!(f, "[req] {path}"),
            LogMsg::ChessSearch { tt_entries, depth, fen } => {
                write!(f, "[chess] TT={tt_entries} entries, depth={depth}, fen={fen}")
            }
            LogMsg::ChessDepth { depth, score, best, is_valid } => {
                write!(f, "[chess]  depth={depth} score={score} best={best} valid={is_valid}")
            }
            LogMsg::ChessNoMove => write!(f, "[chess]  => None (no valid move found across all depths)"),
            LogMsg::ChessResult { best_move } => write!(f, "[chess]  => {best_move}"),
        }
    }
}

pub(crate) static LOG_BUFFER: LazyLock<SegQueue<(LogMsg, LogLevel, DateTime<Utc>)>> =
    LazyLock::new(SegQueue::new);

/// Approximate number of items in `LOG_BUFFER` (used to trigger early flush).
pub(crate) static LOG_DEPTH: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn log_msg(level: LogLevel, msg: LogMsg) {
    LOG_BUFFER.push((msg, level, Utc::now()));
    LOG_DEPTH.fetch_add(1, Ordering::Release);
}

pub(crate) fn log_info(msg: LogMsg) {
    log_msg(LogLevel::Info, msg);
}
pub(crate) fn log_warn(msg: LogMsg) {
    log_msg(LogLevel::Warn, msg);
}
pub(crate) fn log_debug(msg: LogMsg) {
    log_msg(LogLevel::Debug, msg);
}
pub(crate) fn log_error(msg: LogMsg) {
    log_msg(LogLevel::Error, msg);
}

static FILE_CACHE: LazyLock<RwLock<HashMap<&'static str, (StatusCode, String, &'static mime::Mime)>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn main() {
    // ── Panic hook: flush buffered logs before the process goes down ──
    let prev = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let mut buf = String::new();

        while let Some((log_msg, level, time)) = LOG_BUFFER.pop() {
            use std::fmt::Write;

            let _ = writeln!(buf, "[{}] [{:>5}] {}", time.format("%H:%M:%S"), level.to_string(), log_msg);
        }

        if !buf.is_empty() {
            let _ = std::io::stdout().write_all(buf.as_bytes());
            let _ = std::io::stdout().flush();
        }

        prev(info);
    }));

    // ── Background flusher: drain queue to stdout every 500ms,
    //    or immediately when 10 000+ items pile up ──
    let ticker = crossbeam::channel::tick(Duration::from_millis(500));

    std::thread::spawn(move || {
        loop {
            ticker.recv().unwrap();

            loop {
                let mut buf = String::new();

                while let Some((log_msg, level, time)) = LOG_BUFFER.pop() {
                    use std::fmt::Write;

                    let _ = writeln!(buf, "[{}] [{:>5}] {}", time.format("%H:%M:%S"), level.to_string(), log_msg);

                    LOG_DEPTH.fetch_sub(1, Ordering::Release);
                }

                if !buf.is_empty() {
                    print!("{buf}");

                    stdout().flush().ok();
                }

                // If the queue filled up again while we were draining, go again
                // without waiting for the next tick.
                if LOG_DEPTH.load(Ordering::Acquire) > 10_000 {

                    continue;
                }

                break;
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

    LOG_BUFFER.push((LogMsg::Request { path }, LogLevel::Info, time));

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

/// GET /api/games/chess/completions?fen=...&depth=5
///
/// `depth`: 1–12 for anonymous, 13–15 gated behind auth (when implemented).
/// Default 5. Runs on a blocking thread so the async runtime isn't starved.

async fn get_chess_completion(params: Query<HashMap<String, String>>) -> Response<Body> {
    let fen = match params.get("fen") {
        Some(f) => f,
        None => return json_body(StatusCode::BAD_REQUEST, r#"{"error":"missing 'fen' query parameter"}"#.into()),
    };

    let depth = params
        .get("depth")
        .and_then(|v| v.parse::<usize>().ok())
        .map(|d| d.clamp(1, 12))
        .unwrap_or(5);

    // TODO: gate depth 13–15 behind auth token

    let fen = fen.clone();

    let search = tokio::task::spawn_blocking(move || games::chess::best_move(&fen, depth));

    let result = tokio::time::timeout(std::time::Duration::from_secs(60), search).await;

    match result {
        Ok(Ok(Some(best_move))) => json_body(StatusCode::OK, format!(r#"{{"best_move":"{best_move}"}}"#)),
        Ok(Ok(None)) => json_body(StatusCode::BAD_REQUEST, r#"{"error":"no legal moves or invalid FEN"}"#.into()),
        Ok(Err(join_err)) => {
            json_body(StatusCode::INTERNAL_SERVER_ERROR, format!(r#"{{"error":"search panicked: {join_err}"}}"#))
        },
        Err(_elapsed) => json_body(StatusCode::REQUEST_TIMEOUT, r#"{"error":"search timed out after 60s"}"#.into()),
    }
}
