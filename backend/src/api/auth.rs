// ── Auth API route handlers (type-safe Diesel queries) ──

use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
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

use super::bearer_token;

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

/// Uniform error response: `{"error": <msg>}`.
fn err(status: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "error": msg })))
}

/// POST /api/auth/keys — register a FALCON public key
pub async fn register_key(Json(body): Json<RegisterKeyReq>) -> impl IntoResponse {
    if body.key_id.is_empty() || body.key_id.len() > 64 {
        return err(StatusCode::BAD_REQUEST, "key_id must be 1–64 chars");
    }

    let pub_key = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &body.pub_key,
    ) {
        // FN-DSA-1024 only — any other size could register but never verify.
        Ok(b) if b.len() == crate::auth::falcon::FALCON_PUB_KEY_LEN => b,
        Ok(_) => {
            return err(
                StatusCode::BAD_REQUEST,
                "pub_key must be a FALCON-1024 public key (1793 bytes)",
            )
        },
        Err(_) => return err(StatusCode::BAD_REQUEST, "pub_key must be valid base64"),
    };

    let mut conn = match crate::db::connect().await {
        Ok(c) => c,
        Err(e) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: format!("register: DB connect failed: {e}"),
            });
            return err(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        },
    };

    let result = insert_into(tbl::table)
        .values(&NewKey {
            key_id: &body.key_id,
            label: "",
            pub_key: &pub_key,
        })
        // Refuse to overwrite an existing key_id — a re-registration would
        // let an attacker bind their own pubkey to a victim's key_id.
        .on_conflict_do_nothing()
        .execute(&mut conn)
        .await;

    match result {
        Ok(0) => err(StatusCode::CONFLICT, "key_id already registered"),
        Ok(_) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"key_id": body.key_id})),
        ),
        Err(e) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: format!("register: DB insert failed: {e}"),
            });
            err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        },
    }
}

/// GET /api/auth/challenge?key_id=xxx — get a challenge nonce
pub async fn challenge(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let key_id = match params.get("key_id") {
        Some(k) if !k.is_empty() => k,
        _ => return err(StatusCode::BAD_REQUEST, "missing key_id"),
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
        _ => return err(StatusCode::BAD_REQUEST, "nonce must be 64 hex chars"),
    };

    // 2. Verify challenge HMAC (proves server issued this nonce, not expired)
    if !session::verify_challenge_hmac(&body.key_id, &nonce, body.expires_at, &body.hmac_token) {
        return err(StatusCode::UNAUTHORIZED, "invalid or expired challenge");
    }

    // 3. Look up the active key
    let mut conn = match crate::db::connect().await {
        Ok(c) => c,
        Err(e) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: format!("authorize: DB connect failed: {e}"),
            });
            return err(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        },
    };

    let key = match tbl::table
        .filter(tbl::key_id.eq(&body.key_id))
        .filter(tbl::is_active.eq(true))
        .select(KeyRow::as_select())
        .get_result::<KeyRow>(&mut conn)
        .await
    {
        Ok(k) => k,
        // Same generic body as a bad signature — no key enumeration.
        Err(diesel::result::Error::NotFound) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: format!("authorize: unknown or inactive key_id={}", body.key_id),
            });
            return err(StatusCode::UNAUTHORIZED, "authentication failed");
        },
        Err(e) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: format!("authorize: DB select failed: {e}"),
            });
            return err(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        },
    };

    // 4. Decode + verify the FALCON signature over the raw nonce
    let sig = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &body.falcon_sig,
    ) {
        Ok(s) => s,
        Err(_) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: "authorize: falcon_sig base64 decode failed".to_string(),
            });
            return err(StatusCode::UNAUTHORIZED, "authentication failed");
        },
    };

    if !crate::auth::falcon::falcon_verify(&key.pub_key, &nonce, &sig) {
        crate::log_error(crate::LogMsg::Auth {
            detail: format!("authorize: bad FALCON signature for key_id={}", body.key_id),
        });
        return err(StatusCode::UNAUTHORIZED, "authentication failed");
    }

    // 5. Consume the nonce AFTER successful verification (CR-003): failed
    //    attempts must not burn challenges; successes stay single-use.
    if !session::consume_nonce(&nonce, body.expires_at) {
        return err(StatusCode::UNAUTHORIZED, "nonce already used");
    }

    // 6. Issue the stateless session token
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

/// POST /api/auth/logout-all — bump nonce_counter, kill all sessions.
/// Requires a valid session token for the key_id being logged out; without
/// it anyone could revoke an arbitrary key's sessions (session DoS).
pub async fn logout_all(
    headers: HeaderMap,
    Json(body): Json<LogoutAllReq>,
) -> impl IntoResponse {
    let token = match bearer_token(&headers) {
        Some(t) => t,
        None => return err(StatusCode::UNAUTHORIZED, "missing bearer session token"),
    };

    let mut conn = match crate::db::connect().await {
        Ok(c) => c,
        Err(e) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: format!("logout-all: DB connect failed: {e}"),
            });
            return err(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        },
    };

    // Fetch the current counter so the presented token can be verified
    // against it (sessions are stateless; the counter is the revocation key).
    let current_counter = match tbl::table
        .filter(tbl::key_id.eq(&body.key_id))
        .select(tbl::nonce_counter)
        .get_result::<i64>(&mut conn)
        .await
    {
        Ok(c) => c,
        Err(diesel::result::Error::NotFound) => {
            return err(StatusCode::NOT_FOUND, "key_id not found");
        },
        Err(e) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: format!("logout-all: DB select failed: {e}"),
            });
            return err(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        },
    };

    // The token must belong to this key_id with the current counter.
    if !matches!(
        session::parse_session_token(token, current_counter),
        Ok(s) if s.key_id == body.key_id
    ) {
        return err(StatusCode::UNAUTHORIZED, "invalid or expired session token");
    }

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
        Err(diesel::result::Error::NotFound) => err(StatusCode::NOT_FOUND, "key_id not found"),
        Err(e) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: format!("logout-all: DB update failed: {e}"),
            });
            err(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        },
    }
}