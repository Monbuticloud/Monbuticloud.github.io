// Base64 helpers for FALCON keys and signatures (FE-012).
// Chunked conversion avoids the engine's max-arguments limit: FALCON-1024
// private keys are 2305B and signatures up to 1462B.

export function bytesToBinary(bytes) {
  const arr = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  const CHUNK = 0x8000;
  let binary = "";
  for (let i = 0; i < arr.length; i += CHUNK) {
    binary += String.fromCharCode.apply(null, arr.subarray(i, i + CHUNK));
  }
  return binary;
}

export function base64Encode(bytes) {
  return btoa(bytesToBinary(bytes));
}

export function base64Decode(s) {
  // Accept multi-line PEM bodies and missing padding: strip all whitespace
  // (including \r\n) before atob, then restore the "=" padding.
  s = s.replace(/\s+/g, "");
  while (s.length % 4) s += "=";
  const bin = atob(s);
  return Uint8Array.from(bin, (c) => c.charCodeAt(0));
}

export function base64Wrap(b64, width = 64) {
  if (b64.length <= width) return b64;
  let out = "";
  for (let i = 0; i < b64.length; i += width) {
    if (i) out += "\n";
    out += b64.slice(i, i + width);
  }
  return out;
}
