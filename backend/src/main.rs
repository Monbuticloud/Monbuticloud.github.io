mod auth;
mod db;
mod games;
mod schema;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Dedicated thread pool for chess search (tiny stacks, no tokio blocking pool).
/// The engine uses ~70KB stack at peak; 256KB gives 8× margin.
static CHESS_POOL: std::sync::LazyLock<rayon::ThreadPool> = std::sync::LazyLock::new(|| {
    let cpus = std::thread::available_parallelism().map_or(4, |n| n.get());
    rayon::ThreadPoolBuilder::new()
        .num_threads(cpus.max(32).min(128))
        .thread_name(|i| format!("chess-{i}"))
        .stack_size(256 * 1024)
        .build()
        .expect("failed to build chess thread pool")
});

/// Max concurrent chess searches. Beyond this the handler returns 503 immediately.
/// Each search uses 1 leader + 3 helpers = 4 pool threads.
/// Permit count = pool_size / 4 so we never oversubscribe the pool.
/// On a 4‑core machine: pool=32 → 8 permits.
/// On a 128‑core machine: pool=128 → 32 permits.
static CHESS_SEMAPHORE: std::sync::LazyLock<Semaphore> = std::sync::LazyLock::new(|| {
    let cpus = std::thread::available_parallelism().map_or(4, |n| n.get());
    let pool_size = cpus.max(32).min(128);
    Semaphore::new(pool_size / 4)
});

use axum::{
    Router,
    body::Body,
    extract::Query,
    http::{Response, StatusCode, header},
    middleware::{self, Next},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use crossbeam::queue::SegQueue;
use std::{
    collections::HashMap,
    env,
    io::{Write, stdout},
    net::SocketAddr,
    sync::{
        LazyLock, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::runtime::Builder;
use tokio::sync::Semaphore;
use tower_http::services::ServeDir;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogLevel {
    Debug,
    Info,
    Warn,
    #[allow(dead_code)]
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
    Request {
        path: String,
    },
    ChessSearch {
        tt_entries: usize,
        depth: usize,
        fen: String,
    },
    DbPing {
        ok: bool,
        detail: String,
    },
    ChessDepth {
        depth: usize,
        score: i32,
        best: String,
        is_valid: bool,
    },
    ChessNoMove,
    ChessResult {
        best_move: String,
    },
    ResourceUsage {
        rss_kb: u64,
        vm_kb: u64,
        threads: u16,
        cpu_secs: f64,
    },
}

impl std::fmt::Display for LogMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogMsg::Request { path } => write!(f, "[req] {path}"),
            LogMsg::ChessSearch { tt_entries, depth, fen } => {
                write!(f, "[chess] TT={tt_entries} entries, depth={depth}, fen={fen}")
            },
            LogMsg::DbPing { ok, detail } => {
                write!(f, "[db] ping={} detail={detail}", if *ok { "ok" } else { "FAIL" })
            },
            LogMsg::ChessDepth {
                depth,
                score,
                best,
                is_valid,
            } => {
                write!(f, "[chess]  depth={depth} score={score} best={best} valid={is_valid}")
            },
            LogMsg::ChessNoMove => write!(f, "[chess]  => None (no valid move found across all depths)"),
            LogMsg::ChessResult { best_move } => write!(f, "[chess]  => {best_move}"),
            LogMsg::ResourceUsage { rss_kb, vm_kb, threads, cpu_secs } => {
                write!(f, "[res] RSS={rss_kb}kB VM={vm_kb}kB threads={threads} CPU={cpu_secs:.1}s")
            },
        }
    }
}

pub(crate) static LOG_BUFFER: LazyLock<SegQueue<(LogMsg, LogLevel, DateTime<Utc>)>> = LazyLock::new(SegQueue::new);

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
#[allow(dead_code)]

pub(crate) fn log_error(msg: LogMsg) {
    log_msg(LogLevel::Error, msg);
}

static FILE_CACHE: LazyLock<RwLock<HashMap<&'static str, (StatusCode, String, &'static mime::Mime)>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

// ── Resource monitor (reads /proc/self/status and /proc/self/stat) ──

fn sample_resources() -> Option<LogMsg> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let mut rss_kb = 0u64;
    let mut vm_kb = 0u64;
    let mut threads = 0u16;

    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            rss_kb = rest.trim().trim_end_matches("kB").trim().parse().ok()?;
        } else if let Some(rest) = line.strip_prefix("VmSize:") {
            vm_kb = rest.trim().trim_end_matches("kB").trim().parse().ok()?;
        } else if let Some(rest) = line.strip_prefix("Threads:") {
            threads = rest.trim().parse().ok()?;
        }
    }

    // /proc/self/stat: fields 14 (utime) and 15 (stime) at indices 13, 14
    // Comm is in parens — find the last `)` after the first `(` for safety.
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let paren_close = stat.rfind(')')?;
    let rest = &stat[paren_close + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    if fields.len() < 15 {
        return None;
    }
    let utime: u64 = fields[13].parse().ok()?;
    let stime: u64 = fields[14].parse().ok()?;
    let cpu_secs = (utime + stime) as f64 / 100.0; // CLK_TCK = 100 on Linux

    Some(LogMsg::ResourceUsage {
        rss_kb,
        vm_kb,
        threads,
        cpu_secs,
    })
}

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
                let mut flushed: usize = 0;

                while let Some((log_msg, level, time)) = LOG_BUFFER.pop() {
                    use std::fmt::Write;

                    let _ = writeln!(buf, "[{}] [{:>5}] {}", time.format("%H:%M:%S"), level.to_string(), log_msg);

                    LOG_DEPTH.fetch_sub(1, Ordering::Release);
                    flushed += 1;
                }

                if !buf.is_empty() {
                    let now = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
                    print!("{buf}[FLUSHED {flushed} LOGS FROM BUFFER AT {now}]\n\r");

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

    // ── Resource monitor: sample RSS / VM / threads / CPU every 10s ──
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(10));
        if let Some(msg) = sample_resources() {
            log_debug(msg);
        }
    });

    let rt = Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(8192)
        .build()
        .expect("Failed to create Tokio runtime");

    rt.block_on(async {
        let port = env::var("PORT").unwrap_or_else(|_| "8000".to_string());

        let addr: SocketAddr = format!("0.0.0.0:{port}").parse().expect("Invalid socket address");

        let app = Router::new()
            .route("/health", get(|| async { "ok" }))
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
            //- auth tool page
            .route("/auth-tool.html", get(|| serve_static("static/auth.html")))
            .route("/auth-tool", get(|| serve_static("static/auth.html")))
            //- auth API
            .route("/api/auth/keys", post(auth::handlers::register_key))
            .route("/api/auth/challenge", get(auth::handlers::challenge))
            .route("/api/auth/authorize", post(auth::handlers::authorize))
            .route("/api/auth/logout-all", post(auth::handlers::logout_all))
            //-404
            .layer(middleware::from_fn(track_request));

        db::init().await;
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

    let depth = match params.get("depth") {
        Some(v) => match v.parse::<usize>() {
            Ok(d) if (1..=12).contains(&d) => d,
            _ => return json_body(StatusCode::BAD_REQUEST, r#"{"error":"'depth' must be 1–12"}"#.into()),
        },
        None => 5,
    };

    // TODO: gate depth 13–15 behind auth token

    // ── Backpressure: reject immediately if all search slots are busy ──
    let permit = match CHESS_SEMAPHORE.try_acquire() {
        Ok(p) => p,
        Err(_) => {
            return json_body(
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"error":"too many concurrent searches, try again later"}"#.into(),
            );
        },
    };

    let fen = fen.clone();

    let search = tokio::task::spawn_blocking(move || {
        let _permit = permit; // held until search completes
        CHESS_POOL.install(|| games::chess::best_move(&fen, depth))
    });

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
