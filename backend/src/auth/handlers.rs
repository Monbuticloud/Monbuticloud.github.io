// ── Auth route handlers (type-safe Diesel queries) ──
// pub_key field unused until FALCON verification is wired.  Expected dead code.
#![allow(dead_code)]

use axum::{
    extract::Query,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::{
    ExpressionMethods,
    Insertable,
    QueryDsl,
    Queryable,
    Selectable,
    SelectableHelper,
    insert_into,
    update,
};
use diesel_async::RunQueryDsl;
use serde::Deserialize;
use std::collections::HashMap;

use crate::auth::session;
use crate::schema::auth::keys as tbl;

// ── Request types ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterKeyReq {
    pub key_id: String,
    pub pub_key: String, // base64-encoded FALCON public key
}

#[derive(Deserialize)]
pub struct AuthorizeReq {
    pub key_id: String,
    pub nonce: String,       // hex from challenge
    pub hmac_token: String,  // hex from challenge
    pub expires_at: i64,     // timestamp from challenge
    pub falcon_sig: String,  // base64 FALCON signature over nonce
}

#[derive(Deserialize)]
pub struct LogoutAllReq {
    pub key_id: String,
}

// ── Insertable / Queryable rows ──────────────────────────────────

#[derive(Insertable)]
#[diesel(table_name = crate::schema::auth::keys)]
struct NewKey<'a> {
    key_id: &'a str,
    label: &'a str,
    pub_key: &'a [u8],
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::auth::keys)]
struct KeyRow {
    pub_key: Vec<u8>,
    capabilities: i16,
    nonce_counter: i64,
}

// ── Handlers ─────────────────────────────────────────────────────

/// POST /api/auth/keys — register a FALCON public key
pub async fn register_key(Json(body): Json<RegisterKeyReq>) -> impl IntoResponse {
    if body.key_id.is_empty() || body.key_id.len() > 64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "key_id must be 1–64 chars"})),
        );
    }

    let pub_key = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &body.pub_key,
    ) {
        Ok(b) if b.len() <= 4096 => b,
        Ok(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "pub_key too large (max 4096 bytes)"})),
            );
        },
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "pub_key must be valid base64"})),
            );
        },
    };

    let mut conn = match crate::db::connect().await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("DB connection failed: {e}")})),
            );
        },
    };

    let result = insert_into(tbl::table)
        .values(&NewKey {
            key_id: &body.key_id,
            label: "",
            pub_key: &pub_key,
        })
        .on_conflict(tbl::key_id)
        .do_update()
        .set((
            tbl::pub_key.eq(&pub_key),
            tbl::is_active.eq(true),
        ))
        .execute(&mut conn)
        .await;

    match result {
        Ok(_) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"key_id": body.key_id})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB insert failed: {e}")})),
        ),
    }
}

/// GET /api/auth/challenge?key_id=xxx — get a challenge nonce
pub async fn challenge(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let key_id = match params.get("key_id") {
        Some(k) if !k.is_empty() => k,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "missing key_id"})),
            );
        },
    };

    let nonce = session::generate_nonce();
    let expires_at = chrono::Utc::now().timestamp() + 60;
    let hmac_token = session::build_challenge_hmac(key_id, &nonce, expires_at);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "nonce": hex::encode(nonce),
            "hmac_token": hmac_token,
            "expires_at": expires_at,
        })),
    )
}

/// POST /api/auth/authorize — verify FALCON sig, issue session token
pub async fn authorize(Json(body): Json<AuthorizeReq>) -> impl IntoResponse {
    // 1. Parse nonce
    let nonce: [u8; 32] = match hex::decode(&body.nonce) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        },
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "nonce must be 64 hex chars"})),
            );
        },
    };

    // 2. Verify challenge HMAC
    if !session::verify_challenge_hmac(&body.key_id, &nonce, body.expires_at, &body.hmac_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid or expired challenge"})),
        );
    }

    // 3. Look up key from DB
    let mut conn = match crate::db::connect().await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("DB connection failed: {e}")})),
            );
        },
    };

    let key = match tbl::table
        .filter(tbl::key_id.eq(&body.key_id))
        .filter(tbl::is_active.eq(true))
        .select(KeyRow::as_select())
        .get_result::<KeyRow>(&mut conn)
        .await
    {
        Ok(row) => row,
        Err(diesel::result::Error::NotFound) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "unknown or inactive key_id"})),
            );
        },
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("DB lookup failed: {e}")})),
            );
        },
    };

    // 4. Verify FALCON signature
    let sig = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &body.falcon_sig,
    ) {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "falcon_sig must be valid base64"})),
            );
        },
    };

    // TODO: falcon_verify(&key.pub_key, &nonce, &sig)
    //   Add `falcon` crate and wire verification here.
    let _ = sig;

    // 5. Issue session token (1-hour TTL)
    let expiry = session::session_expiry(3600);
    let token = session::build_session_token(&body.key_id, key.nonce_counter, expiry);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "session_token": token,
            "capabilities": key.capabilities,
            "expires_at": expiry,
        })),
    )
}

/// POST /api/auth/logout-all — bump nonce_counter, kill all sessions
pub async fn logout_all(Json(body): Json<LogoutAllReq>) -> impl IntoResponse {
    let mut conn = match crate::db::connect().await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("DB connection failed: {e}")})),
            );
        },
    };

    let result = update(tbl::table.filter(tbl::key_id.eq(&body.key_id)))
        .set(tbl::nonce_counter.eq(tbl::nonce_counter + 1))
        .returning(tbl::nonce_counter)
        .get_result::<i64>(&mut conn)
        .await;

    match result {
        Ok(counter) => (
            StatusCode::OK,
            Json(serde_json::json!({"nonce_counter": counter})),
        ),
        Err(diesel::result::Error::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "key_id not found"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB update failed: {e}")})),
        ),
    }
}
