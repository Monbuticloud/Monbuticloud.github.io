// Key generation, download/load, and registration wiring (FE-012).

import { ui, show } from "./dom.js";
import { state } from "./state.js";
import { api } from "./api.js";
import { base64Decode, base64Encode, base64Wrap } from "./base64.js";
import { generateKeypair } from "./falcon.js";

export function initKeygen() {
  ui.btnGen.addEventListener("click", onGenerate);
  ui.btnDl.addEventListener("click", onDownload);
  ui.btnLoad.addEventListener("click", () => ui.fileIn.click());
  ui.fileIn.addEventListener("change", onFileChange);
  ui.btnReg.addEventListener("click", onRegister);
}

async function onGenerate() {
  const kid = ui.keyId.value.trim();
  if (!kid || kid.length > 64) {
    show(ui.keygenOut, "key_id must be 1–64 chars", "error");
    return;
  }

  ui.btnGen.disabled = true;
  show(ui.keygenOut, "Generating FALCON-1024 keypair (1793B pub / 2305B priv) via falcon-wasm…", "info");

  try {
    state.keypair = await generateKeypair();
    show(
      ui.keygenOut,
      `✅ FALCON-1024 keypair generated (pub ${state.keypair.pubKey.length}B, priv ${state.keypair.privKey.length}B)\n` +
        `pub_key b64: ${base64Encode(state.keypair.pubKey).slice(0, 64)}...`,
      "success",
    );
    ui.btnDl.disabled = false;
    ui.btnReg.disabled = false;
    ui.btnChal.disabled = false;
    ui.falconBanner.classList.add("hidden");
    ui.falconBanner.textContent = "";
  } catch (e) {
    show(ui.keygenOut, `❌ ${e.message}`, "error");
  }

  ui.btnGen.disabled = false;
}

function onDownload() {
  if (!state.keypair) return;

  const kid = ui.keyId.value.trim();
  const header = `-----BEGIN HOMEPAGE FALCON KEY-----\nkey_id: ${kid}\nencrypted: none\n\n`;
  const footer = "\n-----END HOMEPAGE FALCON KEY-----";
  // Wrap at 64 chars per line (PEM convention) — 2305B is ~3074 b64 chars.
  const body = base64Wrap(base64Encode(state.keypair.privKey), 64);
  const content = header + body + footer;

  const blob = new Blob([content], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = "homepage-key.priv";
  a.click();
  URL.revokeObjectURL(url);
}

async function onFileChange(e) {
  const file = e.target.files[0];
  if (!file) return;

  try {
    const text = await file.text();
    const kidMatch = text.match(/^key_id:\s*(.+)$/m);
    if (!kidMatch) {
      show(ui.keygenOut, "Invalid key file: missing key_id", "error");
      return;
    }
    const kid = kidMatch[1].trim();
    if (!kid || kid.length > 64) {
      show(ui.keygenOut, "Invalid key file: key_id must be 1–64 chars", "error");
      return;
    }

    // PEM body is wrapped at 64 chars (~3076 chars for a 2305B key), so join
    // every base64 line between the BEGIN/END markers. Matching a single line
    // (the old `/^[...]+$/m` bug) truncated the key to 48 bytes.
    let b64 = null;
    const pemBlock = text.match(
      /-----BEGIN HOMEPAGE FALCON KEY-----[\s\S]*?-----END HOMEPAGE FALCON KEY-----/,
    );
    if (pemBlock) {
      const inner = pemBlock[0]
        .replace(/-----BEGIN HOMEPAGE FALCON KEY-----/, "")
        .replace(/-----END HOMEPAGE FALCON KEY-----/, "")
        // strip metadata lines (key_id, encrypted, salt, nonce)
        .replace(/^(?:key_id|encrypted|salt|nonce):.*$/gm, "");
      b64 = inner.replace(/\s+/g, "");
    } else {
      // Fallback for files without PEM markers: collect all base64-looking lines.
      b64 = text
        .split(/\r?\n/)
        .map((l) => l.trim())
        .filter((l) => /^[A-Za-z0-9+/=]+$/.test(l) && l.length >= 4)
        .join("");
    }

    if (!b64 || !/^[A-Za-z0-9+/=]+$/.test(b64)) {
      show(ui.keygenOut, "Invalid key file: no valid base64 body found (expected multi-line PEM)", "error");
      return;
    }

    const privBytes = base64Decode(b64);

    // FALCON-1024 only: FN-DSA-512 (1281B) and legacy ECDSA/PKCS8 keys are
    // rejected here because the server only accepts 1793-byte public keys.
    if (privBytes.length !== 2305) {
      show(
        ui.keygenOut,
        `❌ Key is ${privBytes.length}B — FALCON-1024 requires a 2305-byte private key. ` +
          "FN-DSA-512 and legacy ECDSA keys are not supported by this server.",
        "error",
      );
      return;
    }

    state.keypair = { privKey: privBytes, pubKey: new Uint8Array(0) };

    ui.keyId.value = kid;
    ui.btnDl.disabled = false;
    ui.btnChal.disabled = false;
    // A .priv file contains only the private key (public derivation is not
    // exposed by the bundle), so there is nothing new to register.
    ui.btnReg.disabled = true;
    show(
      ui.keygenOut,
      `✅ Loaded FALCON-1024 private key for "${kid}" (2305B).\n` +
        "The file has no public key, so Register stays disabled. Signing works if this key_id is already registered; " +
        "generate a new pair to register a new key.",
      "success",
    );
  } catch (e) {
    show(ui.keygenOut, `❌ ${e.message}`, "error");
  }

  ui.fileIn.value = "";
}

async function onRegister() {
  if (!state.keypair) return;
  if (!state.keypair.pubKey || state.keypair.pubKey.length === 0) {
    show(
      ui.keygenOut,
      "No public key to register — generate a FALCON-1024 keypair first (Load only contains priv).",
      "error",
    );
    return;
  }
  if (state.keypair.pubKey.length !== 1793) {
    show(
      ui.keygenOut,
      `Pub key is ${state.keypair.pubKey.length}B — server requires FALCON-1024 (1793B). Generate via falcon-wasm.`,
      "error",
    );
    return;
  }
  const kid = ui.keyId.value.trim();
  if (!kid || kid.length > 64) {
    show(ui.keygenOut, "key_id must be 1–64 chars", "error");
    return;
  }
  ui.btnReg.disabled = true;
  show(ui.keygenOut, "Registering FALCON-1024 pub key (1793B)…", "info");

  const res = await api("POST", "/api/auth/keys", {
    key_id: kid,
    pub_key: base64Encode(state.keypair.pubKey),
  });

  if (res.ok) {
    show(ui.keygenOut, `✅ Registered "${res.data.key_id}" (FALCON-1024)`, "success");
  } else {
    show(ui.keygenOut, `❌ ${res.data.error || `HTTP ${res.status}`}`, "error");
    ui.btnReg.disabled = false;
  }
}
