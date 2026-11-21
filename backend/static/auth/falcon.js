// FALCON-1024 WASM loader + keygen/sign wrappers (FE-012).
// The backend serves /falcon_wasm.js and /falcon_wasm_bg.wasm; there is no
// Web Crypto/ECDSA fallback because the server only accepts FN-DSA-1024.

import { state } from "./state.js";
import { ui } from "./dom.js";

let falcon = null;
let falconReady = false;
let falconError = null;
let falconInitPromise = null;

function showFalconBanner(message) {
  if (!ui.falconBanner) return;
  // textContent, not innerHTML: the message contains engine error text.
  ui.falconBanner.textContent = message;
  ui.falconBanner.classList.remove("hidden");
}

function hideFalconBanner() {
  if (!ui.falconBanner) return;
  ui.falconBanner.classList.add("hidden");
  ui.falconBanner.textContent = "";
}

async function loadFalcon() {
  try {
    // wasm-bindgen glue; its default initializer fetches the .wasm relative
    // to this module URL. Both paths are STATIC_ROUTES in src/main.rs.
    const mod = await import("/falcon_wasm.js");
    if (typeof mod.default !== "function") {
      throw new Error("falcon_wasm.js is missing the wasm-bindgen initializer");
    }
    await mod.default();
    if (typeof mod.generate_keypair !== "function" || typeof mod.sign !== "function") {
      throw new Error("falcon_wasm.js is missing generate_keypair/sign exports");
    }
    falcon = mod;
    falconReady = true;
    falconError = null;
    hideFalconBanner();
    return true;
  } catch (e) {
    falcon = null;
    falconReady = false;
    falconError = e?.message || String(e);
    const reason =
      location.protocol === "file:"
        ? "This page was opened from file:// (offline/MHTML) and cannot fetch the WASM bundle."
        : "The backend must serve /falcon_wasm.js and /falcon_wasm_bg.wasm.";
    showFalconBanner(
      `FALCON WASM not loaded — ${falconError}\n` +
        "This tool only signs with FN-DSA-1024 (FALCON-1024, 1793-byte public keys). " +
        "The server rejects non-1024 keys; no ECDSA fallback is available.\n" +
        `${reason}\n` +
        "Rebuild with backend/crates/falcon-wasm/build.sh and serve the pkg/ files from the backend.",
    );
    return false;
  }
}

export function initFalcon() {
  if (falconReady && falcon) return Promise.resolve(true);
  // Deduplicate concurrent attempts (the page kicks off init at load time and
  // the first user click may arrive before it finishes).
  if (!falconInitPromise) {
    falconInitPromise = loadFalcon().finally(() => {
      falconInitPromise = null;
    });
  }
  return falconInitPromise;
}

export async function generateKeypair() {
  if (!falconReady) {
    const ok = await initFalcon();
    if (!ok) {
      throw new Error(
        falconError
          ? `FALCON WASM not ready: ${falconError}. Cannot generate a 1793-byte FALCON-1024 key.`
          : "FALCON WASM not loaded — serve falcon_wasm_bg.wasm or rebuild the pkg/ bundle.",
      );
    }
  }

  // FALCON-1024 is logn=10 (NIST Level V). Keygen is CPU-heavy and
  // synchronous inside WASM; yield once so the status message paints.
  await new Promise((resolve) => setTimeout(resolve, 0));

  const kp = falcon.generate_keypair(10);
  // wasm-bindgen returns { public_key: Uint8Array, private_key: Uint8Array }
  const pubKey = kp.public_key ?? kp.publicKey ?? kp[0];
  const privKey = kp.private_key ?? kp.privateKey ?? kp[1];
  if (!pubKey || !privKey) throw new Error("FALCON generate_keypair returned an unexpected shape");

  const pub = new Uint8Array(pubKey);
  const priv = new Uint8Array(privKey);

  // Refuse to hand a non-FALCON-1024 keypair to the UI — the server only
  // accepts 1793-byte public keys and verification would fail anyway.
  if (pub.length !== 1793 || priv.length !== 2305) {
    throw new Error(
      `FALCON-1024 key size mismatch: pub ${pub.length}B (want 1793), priv ${priv.length}B (want 2305)`,
    );
  }

  return { pubKey: pub, privKey: priv };
}

export async function signNonce(nonceBytes) {
  if (!state.keypair?.privKey) throw new Error("No key loaded for signing");
  if (state.keypair.privKey.length !== 2305) {
    throw new Error(`Private key is ${state.keypair.privKey.length}B — FALCON-1024 requires 2305B`);
  }
  if (!falconReady) {
    const ok = await initFalcon();
    if (!ok) {
      throw new Error(
        falconError ? `FALCON WASM not ready: ${falconError}` : "FALCON WASM not loaded — cannot sign.",
      );
    }
  }
  // falcon.sign(private_key: Uint8Array, message: Uint8Array) -> Uint8Array
  const sig = falcon.sign(state.keypair.privKey, nonceBytes);
  const out = sig instanceof Uint8Array ? sig : new Uint8Array(sig);
  if (out.length === 0) throw new Error("FALCON sign returned an empty signature");
  return out;
}
