// ── Static file serving (runtime cache — nginx shadows /static/assets in prod) ──
// FE-003: CSS files are flattened at Docker image-time via postcss-preset-env
// (static/*.css → static/dist/*.css). The runtime image ships only the
// flattened dist; a local `cargo run` without a build falls back to the
// nested source. Source files remain the authors' input.

use axum::{
    body::Body,
    http::{Response, StatusCode, header},
};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

/// Cache key: request path plus whether the bytes came from `static/dist`.
/// Including the origin keeps a flattened response from sharing a slot with
/// a source fallback (or vice versa).
type CacheKey = (&'static str, bool);
type FileCache = LazyLock<RwLock<HashMap<CacheKey, (StatusCode, String, &'static mime::Mime)>>>;
static FILE_CACHE: FileCache = LazyLock::new(|| RwLock::new(HashMap::new()));

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

/// Resolve the preferred file to serve. For CSS, prefer `static/dist/<file>`
/// (flattened by postcss at `docker build`) when it exists on disk; otherwise
/// fall back to the source path for local development. Non-CSS paths pass
/// through unchanged. Returns `(status, content, from_dist)`.
async fn resolve_content(path: &'static str) -> (StatusCode, String, bool) {
    // FE-003: Prefer dist for CSS.
    if path.ends_with(".css") && path.starts_with("static/") {
        let basename = path.rsplit('/').next().unwrap_or(path);
        let dist_path = format!("static/dist/{basename}");
        if let Ok(c) = tokio::fs::read_to_string(&dist_path).await {
            return (StatusCode::OK, c, true);
        }
        // Dist not found — fall back to source (local dev without `npm run build:css`).
    }

    match tokio::fs::read_to_string(path).await {
        Ok(c) => (StatusCode::OK, c, false),
        Err(_) => (StatusCode::NOT_FOUND, "<h1>Not Found</h1>".into(), false),
    }
}

#[inline(always)]
pub async fn serve_static(path: &'static str) -> Response<Body> {
    // Recover from a poisoned lock instead of cascading panics into every
    // subsequent handler (CR-010): the guard's data is still valid.
    // Prefer a dist-cached CSS response; fall back to a source-cached one.
    let cached = {
        let cache = FILE_CACHE.read().unwrap_or_else(|p| p.into_inner());
        cache.get(&(path, true)).or_else(|| cache.get(&(path, false))).cloned()
    };

    if let Some((status, content, mime)) = cached {
        return Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content))
            .unwrap();
    }

    let mime = mime_for_path(path);

    let (status, content, from_dist) = resolve_content(path).await;

    // Only cache successful reads — a transient miss (file not yet mounted)
    // must not be pinned as a permanent error.
    if status == StatusCode::OK {
        FILE_CACHE
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert((path, from_dist), (status, content.clone(), mime));
    }

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(content))
        .unwrap()
}
