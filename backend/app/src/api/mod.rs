// ── HTTP API: one file per concern ──

use axum::http::{HeaderMap, header};

pub mod auth;
pub mod chess;
pub mod html;

/// Extract a "Bearer <token>" from the Authorization header.
pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let auth = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix("Bearer ").map(str::trim).filter(|t| !t.is_empty())
}
