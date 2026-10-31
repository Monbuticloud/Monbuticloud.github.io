// ── Static file serving (runtime cache — nginx shadows /static/assets in prod) ──

use axum::{
    body::{Body, Bytes},
    http::{Response, StatusCode, header},
};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

// Contents are cached as `Bytes` because static routes also serve binary
// payloads (the FALCON WASM bundle) — `String` would fail UTF-8 decoding.
type FileCache = LazyLock<RwLock<HashMap<&'static str, (StatusCode, Bytes, &'static str)>>>;

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

#[inline(always)]
pub async fn serve_static(path: &'static str) -> Response<Body> {
    // Recover from a poisoned lock instead of cascading panics into every
    // subsequent handler (CR-010): the guard's data is still valid.
    if let Some(cached) = FILE_CACHE
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .get(path)
    {
        let (status, content, mime) = cached;

        return Response::builder()
            .status(*status)
            .header(header::CONTENT_TYPE, *mime)
            .body(Body::from(content.clone()))
            .unwrap();
    }

    let mime = mime_for_path(path);

    let (status, content) = match tokio::fs::read(path).await {
        Ok(c) => (StatusCode::OK, Bytes::from(c)),
        Err(_) => (StatusCode::NOT_FOUND, Bytes::from_static(b"<h1>Not Found</h1>")),
    };

    // Only cache successful reads — a transient miss (file not yet mounted)
    // must not be pinned as a permanent error.
    if status == StatusCode::OK {
        FILE_CACHE
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(path, (status, content.clone(), mime));
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
        assert_eq!(mime_for_path("static/auth.html"), "text/html; charset=utf-8");
        assert_eq!(mime_for_path("static/style.css"), "text/css; charset=utf-8");
        assert_eq!(
            mime_for_path("static/auth.js"),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(mime_for_path("static/robots.txt"), "text/plain");
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
