// ── Auth module: FALCON + HMAC challenge-response with stateless sessions ──
// HTTP route handlers live in `crate::api::auth`; this module holds the
// non-HTTP session/HMAC logic.

/// Capability bits stored in `auth.keys.capabilities` (SMALLINT).
/// Bit assignment is first-come; the schema default of 15 sets all low bits.
pub const CAP_DEEP_SEARCH: i16 = 0b0000_0001;

pub mod falcon;
pub mod session;
