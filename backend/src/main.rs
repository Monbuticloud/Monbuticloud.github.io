mod api;
mod auth;
mod db;
mod games;
mod schema;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use axum::{
    Router,
    body::Body,
    http::Response,
    middleware::{self, Next},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use crossbeam::queue::SegQueue;
use std::{
    env,
    io::{Write, stdout},
    net::SocketAddr,
    sync::{
        LazyLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::runtime::Builder;
use tower_http::services::ServeDir;

use api::html::serve_static;

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
    Auth {
        detail: String,
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
            LogMsg::Auth { detail } => write!(f, "[auth] {detail}"),
            LogMsg::ResourceUsage {
                rss_kb,
                vm_kb,
                threads,
                cpu_secs,
            } => {
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

/// Static page routes: (URL path, file). Compiled in — one source of truth,
/// iterated at startup instead of ~20 hand-written `.route()` calls.
/// CWD-relative so it works in local dev (cargo run) and in the container
/// (WORKDIR-less scratch → CWD=/). In production nginx serves /static/assets/
/// directly and shadows the nest_service route.
const STATIC_ROUTES: &[(&str, &str)] = &[
    ("/", "static/index.html"),
    ("/index", "static/index.html"),
    ("/index.html", "static/index.html"),
    ("/script.js", "static/script.js"),
    ("/style.css", "static/style.css"),
    ("/contact", "static/contact.html"),
    ("/contact.html", "static/contact.html"),
    ("/contact.js", "static/contact.js"),
    ("/contact.css", "static/contact.css"),
    ("/projects", "static/projects.html"),
    ("/projects.html", "static/projects.html"),
    ("/projects.js", "static/projects.js"),
    ("/projects.css", "static/projects.css"),
    ("/common.js", "static/common.js"),
    ("/common.css", "static/common.css"),
    ("/auth-tool.html", "static/auth.html"),
    ("/auth-tool", "static/auth.html"),
];

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
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(Duration::from_secs(10));
            if let Some(msg) = sample_resources() {
                log_debug(msg);
            }
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

        let mut app = Router::new()
            .route("/health", get(|| async { "ok" }))
            //-static assets
            // CWD-relative so it works in local dev (cargo run) and in the
            // container (WORKDIR-less scratch → CWD=/). In production nginx
            // serves /static/assets/ directly and shadows this route.
            .nest_service("/static/assets", ServeDir::new("static/assets"))
            //- chess API
            .route("/api/games/chess/completions", get(api::chess::get_chess_completion))
            //- auth API
            .route("/api/auth/keys", post(api::auth::register_key))
            .route("/api/auth/challenge", get(api::auth::challenge))
            .route("/api/auth/authorize", post(api::auth::authorize))
            .route("/api/auth/logout-all", post(api::auth::logout_all));

        //- static pages (const table: path → file)
        for &(path, file) in STATIC_ROUTES {
            app = app.route(path, get(move || serve_static(file)));
        }

        //-404
        let app = app.layer(middleware::from_fn(track_request));

        db::init().await;
        println!("listening on {addr}");

        stdout().flush().ok();

        let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to bind");

        axum::serve(listener, app).await.expect("Server error");
    });
}

async fn track_request(request: axum::http::Request<Body>, next: Next) -> Response<Body> {
    let path = request.uri().path().to_owned();

    let time = Utc::now();

    LOG_BUFFER.push((LogMsg::Request { path }, LogLevel::Info, time));

    next.run(request).await
}
