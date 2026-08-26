// ── Static file serving (runtime cache — nginx shadows /static/assets in prod) ──

use axum::{
    body::Body,
    http::{Response, StatusCode, header},
};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

type FileCache = LazyLock<RwLock<HashMap<&'static str, (StatusCode, String, &'static mime::Mime)>>>;

static FILE_CACHE: FileCache =
    LazyLock::new(|| RwLock::new(HashMap::new()));

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
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.clone()))
            .unwrap();
    }

    let mime = mime_for_path(path);

    let (status, content) = match tokio::fs::read_to_string(path).await {
        Ok(c) => (StatusCode::OK, c),
        Err(_) => (StatusCode::NOT_FOUND, "<h1>Not Found</h1>".into()),
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
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(content))
        .unwrap()
}
