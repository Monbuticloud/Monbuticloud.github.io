// ── Chess API handler ──

use axum::{
    body::Body,
    extract::Query,
    http::{Response, StatusCode, header},
};
use serde::Deserialize;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::Semaphore;

use crate::games::chess;

/// Dedicated thread pool for chess search (tiny stacks, no tokio blocking pool).
/// The engine uses ~70KB stack at peak; 256KB gives 8× margin.
/// Sized to the available cores: each search spawns 1 leader + up to 3
/// helpers, and the semaphore below caps concurrent searches at pool/4, so
/// every permitted search's tasks run at once without oversubscribing the CPU.
static CHESS_POOL: LazyLock<rayon::ThreadPool> = LazyLock::new(|| {
    let cpus = std::thread::available_parallelism().map_or(2, |n| n.get()).max(1);
    rayon::ThreadPoolBuilder::new()
        .num_threads(cpus)
        .thread_name(|i| format!("chess-{i}"))
        .stack_size(256 * 1024)
        .build()
        .expect("failed to build chess thread pool")
});

/// Max concurrent chess searches. Beyond this the handler returns 503 immediately.
/// Each search uses 1 leader + 3 helpers = 4 pool threads.
/// Permit count = pool_size / 4 so we never oversubscribe the pool.
/// On a 4-core machine: pool=4 → 1 permit.
/// On a 128-core machine: pool=128 → 32 permits.
static CHESS_SEMAPHORE: LazyLock<Semaphore> = LazyLock::new(|| {
    let cpus = std::thread::available_parallelism().map_or(2, |n| n.get()).max(1);
    Semaphore::new((cpus / 4).max(1))
});

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
#[derive(Deserialize)]
pub struct ChessQuery {
    fen: Option<String>,
    depth: Option<usize>,
}

pub async fn get_chess_completion(Query(q): Query<ChessQuery>) -> Response<Body> {
    let fen = match q.fen {
        Some(f) => f,
        None => return json_body(StatusCode::BAD_REQUEST, r#"{"error":"missing 'fen' query parameter"}"#.into()),
    };

    let depth = match q.depth {
        Some(d) if (1..=15).contains(&d) => d,
        Some(_) => return json_body(StatusCode::BAD_REQUEST, r#"{"error":"'depth' must be 1–15"}"#.into()),
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

    let search = tokio::task::spawn_blocking(move || {
        let _permit = permit; // held until search completes
        CHESS_POOL.install(|| chess::best_move(&fen, depth))
    });

    let result = tokio::time::timeout(Duration::from_secs(60), search).await;

    match result {
        Ok(Ok(Some(best_move))) => json_body(StatusCode::OK, format!(r#"{{"best_move":"{best_move}"}}"#)),
        Ok(Ok(None)) => json_body(StatusCode::BAD_REQUEST, r#"{"error":"no legal moves or invalid FEN"}"#.into()),
        Ok(Err(join_err)) => {
            json_body(StatusCode::INTERNAL_SERVER_ERROR, format!(r#"{{"error":"search panicked: {join_err}"}}"#))
        },
        Err(_elapsed) => json_body(StatusCode::REQUEST_TIMEOUT, r#"{"error":"search timed out after 60s"}"#.into()),
    }
}