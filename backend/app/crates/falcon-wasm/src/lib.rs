// ── FALCON WASM wrapper (FN-DSA FIPS 206) ──
//
// Exports three functions for browser use:
//   generate_keypair(logn) → { public_key, private_key }
//   sign(private_key, message) → signature
//   verify(public_key, message, signature) → bool
//
// Build:
//   cargo build --target wasm32-unknown-unknown --release
//   wasm-bindgen target/wasm32-unknown-unknown/release/falcon_wasm.wasm \
//       --out-dir pkg --target web
//   wasm-opt -Oz -o pkg/falcon_wasm_bg.wasm pkg/falcon_wasm_bg.wasm

use falcon::prelude::*;
use wasm_bindgen::prelude::*;

// ── Panic hook ───────────────────────────────────────────────────

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

// ── Key generation ──────────────────────────────────────────────

/// Generate a FALCON keypair.
///
/// `logn` — security level:
///   9  → FN-DSA-512  (NIST Level I,   pub 897B,  priv 1281B)
///   10 → FN-DSA-1024 (NIST Level V,   pub 1793B, priv 2305B)
///
/// Returns `{ public_key: Uint8Array, private_key: Uint8Array }`.
#[wasm_bindgen]
pub fn generate_keypair(logn: u8) -> Result<js_sys::Object, JsError> {
    let kp = FnDsaKeyPair::generate(logn.into())?;

    let pub_key = kp.public_key().to_vec();
    let priv_key = kp.private_key().to_vec();

    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &JsValue::from("public_key"), &js_sys::Uint8Array::from(&pub_key[..]))
        .map_err(|e| JsError::new(&format!("Reflect.set failed: {e:?}")))?;
    js_sys::Reflect::set(&obj, &JsValue::from("private_key"), &js_sys::Uint8Array::from(&priv_key[..]))
        .map_err(|e| JsError::new(&format!("Reflect.set failed: {e:?}")))?;

    Ok(obj)
}

// ── Sign ────────────────────────────────────────────────────────

/// Sign a message with a FALCON private key.
///
/// `private_key` — raw private key bytes (1281 for FN-DSA-512, 2305 for FN-DSA-1024)
/// `message`     — the bytes to sign
/// Returns the signature as `Uint8Array` (666 bytes for FN-DSA-512).
#[wasm_bindgen]
pub fn sign(private_key: &[u8], message: &[u8]) -> Result<js_sys::Uint8Array, JsError> {
    let kp = FnDsaKeyPair::from_private_key(private_key)?;
    let sig = kp.sign(message, &DomainSeparation::None)?;
    Ok(js_sys::Uint8Array::from(&sig.to_bytes()[..]))
}

// ── Verify ──────────────────────────────────────────────────────

/// Verify a FALCON signature.
///
/// `public_key` — raw public key bytes (897 for FN-DSA-512, 1793 for FN-DSA-1024)
/// `message`    — the original signed bytes
/// `signature`  — the signature bytes (666 for FN-DSA-512)
/// Returns `true` if the signature is valid.
#[wasm_bindgen]
pub fn verify(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    match FnDsaSignature::verify(signature, public_key, message, &DomainSeparation::None) {
        Ok(()) => true,
        Err(_) => false,
    }
}
