// Shared auth-tool state. Kept in one mutable object so the modules that
// read/write it do not need circular imports (FE-012).

export const state = {
  keypair: null, // { pubKey: Uint8Array(1793), privKey: Uint8Array(2305) }
  challenge: null, // { nonce, hmac_token, expires_at }
  sessionToken: null,
  sessionData: null, // { capabilities, expires_at }
};
