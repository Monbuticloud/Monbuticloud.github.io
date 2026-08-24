// ── Auth module: FALCON + HMAC challenge-response with stateless sessions ──
// HTTP route handlers live in `crate::api::auth`; this module holds the
// non-HTTP session/HMAC logic.

pub mod falcon;
pub mod session;
