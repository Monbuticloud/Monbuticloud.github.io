// ── FALCON (FN-DSA FIPS 206) signature verification ──
//
// Server-side counterpart to crates/falcon-wasm. Signatures are produced
// client-side with DomainSeparation::None over the raw challenge nonce;
// verification here must match exactly.

use falcon::prelude::*;

/// FN-DSA-1024 public key size in bytes.
pub const FALCON_PUB_KEY_LEN: usize = 1793;

/// Verify a FALCON-1024 signature over `message`.
/// Returns false for any malformed input — never panics.
pub fn falcon_verify(pub_key: &[u8], message: &[u8], sig: &[u8]) -> bool {
    if pub_key.len() != FALCON_PUB_KEY_LEN {
        return false; // FN-DSA-1024 keys only
    }
    FnDsaSignature::verify(sig, pub_key, message, &DomainSeparation::None).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signing a challenge requires a private key; tests generate one.
    /// Slow (~seconds) but runs once per test binary.
    fn make_keypair() -> (Vec<u8>, Vec<u8>) {
        let kp = FnDsaKeyPair::generate(10).expect("FN-DSA-1024 keypair");
        (kp.public_key().to_vec(), kp.private_key().to_vec())
    }

    #[test]
    fn valid_signature_verifies() {
        let (pk, sk) = make_keypair();
        let kp = FnDsaKeyPair::from_private_key(&sk).unwrap();
        let msg = [0xABu8; 32];
        let sig = kp.sign(&msg, &DomainSeparation::None).unwrap();
        assert!(falcon_verify(&pk, &msg, &sig.to_bytes()));
    }

    #[test]
    fn wrong_message_fails() {
        let (pk, sk) = make_keypair();
        let kp = FnDsaKeyPair::from_private_key(&sk).unwrap();
        let sig = kp.sign(&[1u8; 32], &DomainSeparation::None).unwrap();
        assert!(!falcon_verify(&pk, &[2u8; 32], &sig.to_bytes()));
    }

    #[test]
    fn tampered_signature_fails() {
        let (pk, sk) = make_keypair();
        let kp = FnDsaKeyPair::from_private_key(&sk).unwrap();
        let msg = [3u8; 32];
        let sig = kp.sign(&msg, &DomainSeparation::None).unwrap();
        let mut bytes = sig.to_bytes().to_vec();
        bytes[0] ^= 0xFF;
        assert!(!falcon_verify(&pk, &msg, &bytes));
    }

    #[test]
    fn wrong_pubkey_length_rejected_before_crypto() {
        // 897 = FN-DSA-512 size; must fail the length guard, not panic.
        assert!(!falcon_verify(&[0u8; 897], &[0u8; 32], &[0u8; 64]));
        assert!(!falcon_verify(&[], &[0u8; 32], &[0u8; 64]));
    }
}
