// ── Static file serving (runtime cache — nginx shadows /static/assets in prod) ──
// FE-003: CSS files are flattened at Docker image-time via postcss-preset-env
// (frontend/static/*.css → frontend/static/dist/*.css). The runtime image ships only the
// flattened dist; a local `cargo run` without a build falls back to the
// nested source. Source files remain the authors' input.

use axum::{
    body::{Body, Bytes},
    http::{Response, StatusCode, header},
};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

// Contents are cached as `Bytes` because static routes also serve binary
// payloads (the FALCON WASM bundle) — `String` would fail UTF-8 decoding.
/// Cache key: request path plus whether the bytes came from `frontend/static/dist`.
/// Including the origin keeps a flattened response from sharing a slot with
/// a source fallback (or vice versa).
type CacheKey = (&'static str, bool);
type FileCache = LazyLock<RwLock<HashMap<CacheKey, (StatusCode, Bytes, &'static str)>>>;

static FILE_CACHE: FileCache =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn mime_for_path(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".wasm") {
        // Required for `WebAssembly.instantiateStreaming` in wasm-bindgen.
        "application/wasm"
    } else {
        "text/plain"
    }
}

/// Resolve the preferred file to serve. For CSS, prefer `frontend/static/dist/<file>`
/// (flattened by postcss at `docker build`) when it exists on disk; otherwise
/// fall back to the source path for local development. Non-CSS paths pass
/// through unchanged. Returns `(status, content, from_dist)`.
async fn resolve_content(path: &'static str) -> (StatusCode, Bytes, bool) {
    // FE-003: Prefer dist for CSS.
    if path.ends_with(".css") && path.starts_with("frontend/static/") {
        let basename = path.rsplit('/').next().unwrap_or(path);
        let dist_path = format!("frontend/static/dist/{basename}");
        if let Ok(c) = tokio::fs::read(&dist_path).await {
            return (StatusCode::OK, Bytes::from(c), true);
        }
        // Dist not found — fall back to source (local dev without `npm run build:css`).
    }

    match tokio::fs::read(path).await {
        Ok(c) => (StatusCode::OK, Bytes::from(c), false),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Bytes::from_static(b"<h1>Not Found</h1>"),
            false,
        ),
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
            .header(header::CONTENT_TYPE, mime)
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
        .header(header::CONTENT_TYPE, mime)
        .body(Body::from(content))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::{mime_for_path, serve_static};
    use axum::http::{StatusCode, header};

    #[test]
    fn wasm_is_served_with_application_wasm() {
        assert_eq!(
            mime_for_path("crates/falcon-wasm/pkg/falcon_wasm_bg.wasm"),
            "application/wasm"
        );
    }

    #[test]
    fn text_assets_keep_their_types() {
        assert_eq!(mime_for_path("frontend/static/auth.html"), "text/html; charset=utf-8");
        assert_eq!(mime_for_path("frontend/static/style.css"), "text/css; charset=utf-8");
        assert_eq!(
            mime_for_path("frontend/static/auth.js"),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(mime_for_path("frontend/static/robots.txt"), "text/plain");
    }

    /// The auth tool imports the wasm-bindgen glue, which then fetches the
    /// binary. Serving it through `read_to_string` used to 404 it — assert the
    /// bytes round-trip unchanged and the MIME enables instantiateStreaming.
    #[tokio::test]
    async fn serves_wasm_bytes_unchanged() {
        const WASM_PATH: &str =
            concat!(env!("CARGO_MANIFEST_DIR"), "/crates/falcon-wasm/pkg/falcon_wasm_bg.wasm");

        let expected = tokio::fs::read(WASM_PATH).await.expect("tracked pkg wasm");
        let res = serve_static(WASM_PATH).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "application/wasm");

        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        assert_eq!(&body[..], &expected[..]);
    }
}
