// Challenge-response, signing, and logout wiring (FE-012).

import { ui, show } from "./dom.js";
import { state } from "./state.js";
import { api } from "./api.js";
import { base64Encode } from "./base64.js";
import { signNonce } from "./falcon.js";

export function initSession() {
  ui.btnChal.addEventListener("click", onChallenge);
  ui.btnSign.addEventListener("click", onSign);
  ui.btnLo.addEventListener("click", onLogout);
}

async function onChallenge() {
  const kid = ui.keyId.value.trim();
  if (!kid || kid.length > 64) {
    show(ui.authOut, "key_id must be 1–64 chars", "error");
    return;
  }
  ui.btnChal.disabled = true;
  show(ui.authOut, "Requesting challenge…", "info");

  const res = await api("GET", `/api/auth/challenge?key_id=${encodeURIComponent(kid)}`);

  if (res.ok) {
    state.challenge = res.data;
    show(ui.authOut, `✅ Challenge received\nnonce: ${state.challenge.nonce.slice(0, 32)}...`, "success");
    ui.btnSign.disabled = false;
  } else {
    show(ui.authOut, `❌ ${res.data.error || `HTTP ${res.status}`}`, "error");
    ui.btnChal.disabled = false;
  }
}

async function onSign() {
  if (!state.challenge || !state.keypair) return;
  ui.btnSign.disabled = true;
  show(ui.authOut, "Signing nonce with FALCON-1024...", "info");

  try {
    // challenge.nonce is hex (64 chars → 32 bytes). Decode hex directly.
    const h = state.challenge.nonce;
    if (!/^[0-9a-fA-F]+$/.test(h) || h.length % 2 !== 0) throw new Error("Invalid nonce hex");
    const nonceBytes = new Uint8Array(h.length / 2);
    for (let i = 0; i < nonceBytes.length; i++) {
      nonceBytes[i] = parseInt(h.slice(i * 2, i * 2 + 2), 16);
    }

    const sigBytes = await signNonce(nonceBytes);

    show(ui.authOut, `Authorizing (sig ${sigBytes.length}B)...`, "info");

    const res = await api("POST", "/api/auth/authorize", {
      key_id: ui.keyId.value.trim(),
      nonce: state.challenge.nonce,
      hmac_token: state.challenge.hmac_token,
      expires_at: state.challenge.expires_at,
      falcon_sig: base64Encode(sigBytes),
    });

    if (res.ok) {
      state.sessionToken = res.data.session_token;
      state.sessionData = { capabilities: res.data.capabilities, expires_at: res.data.expires_at };
      ui.sessTok.textContent = state.sessionToken.slice(0, 48) + "...";
      ui.sessCap.textContent = state.sessionData.capabilities;
      ui.sessExp.textContent = new Date(state.sessionData.expires_at * 1000).toLocaleString();
      ui.sessInfo.classList.remove("hidden");
      ui.btnApi.disabled = false;
      show(ui.authOut, `✅ Authorized! Capabilities: ${state.sessionData.capabilities}`, "success");
      ui.btnChal.disabled = false;
    } else {
      show(ui.authOut, `❌ ${res.data.error || `HTTP ${res.status}`}`, "error");
      ui.btnSign.disabled = false;
      ui.btnChal.disabled = false;
    }
  } catch (e) {
    show(ui.authOut, `❌ ${e.message}`, "error");
    ui.btnSign.disabled = false;
    ui.btnChal.disabled = false;
  }
}

async function onLogout() {
  ui.btnLo.disabled = true;
  const res = await api("POST", "/api/auth/logout-all", {
    key_id: ui.keyId.value.trim(),
  });

  if (res.ok) {
    state.sessionToken = null;
    state.sessionData = null;
    ui.sessInfo.classList.add("hidden");
    ui.btnApi.disabled = true;
    show(ui.authOut, `✅ Logged out all sessions (counter: ${res.data.nonce_counter})`, "success");
  } else {
    show(ui.authOut, `❌ ${res.data.error || `HTTP ${res.status}`}`, "error");
  }

  ui.btnLo.disabled = false;
}
