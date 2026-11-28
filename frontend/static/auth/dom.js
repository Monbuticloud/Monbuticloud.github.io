// DOM lookup + status rendering for the auth tool (FE-012).

export const $ = (id) => document.getElementById(id);

export const ui = {
  keyId: $("key-id"),
  btnGen: $("btn-generate"),
  btnDl: $("btn-download-key"),
  btnLoad: $("btn-load-key"),
  btnReg: $("btn-register"),
  fileIn: $("file-input"),
  btnChal: $("btn-challenge"),
  btnSign: $("btn-sign"),
  btnLo: $("btn-logout-all"),
  btnApi: $("btn-call-api"),
  keygenOut: $("keygen-output"),
  authOut: $("auth-output"),
  apiOut: $("api-output"),
  sessInfo: $("session-info"),
  sessTok: $("sess-token"),
  sessCap: $("sess-cap"),
  sessExp: $("sess-exp"),
  falconBanner: $("falcon-banner"),
};

/**
 * Replace a status element's text and state class.
 * @param {HTMLElement} el
 * @param {string} msg
 * @param {"error"|"success"|"info"|undefined} [cls]
 */
export function show(el, msg, cls) {
  el.classList.remove("hidden", "error", "success", "info");
  el.textContent = msg;
  if (cls) el.classList.add(cls);
}
