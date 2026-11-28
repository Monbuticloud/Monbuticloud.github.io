// JSON API wrapper for the auth tool (FE-012).

import { state } from "./state.js";

const API_BASE = "";

/**
 * @param {"GET"|"POST"} method
 * @param {string} path
 * @param {object} [body]
 * @returns {Promise<{ ok: boolean, status: number, data: any }>}
 */
export async function api(method, path, body) {
  const opts = { method, headers: {} };
  if (body) {
    opts.headers["Content-Type"] = "application/json";
    opts.body = JSON.stringify(body);
  }
  if (state.sessionToken) {
    opts.headers["Authorization"] = `Bearer ${state.sessionToken}`;
  }

  let res;
  try {
    res = await fetch(API_BASE + path, opts);
  } catch (e) {
    return { ok: false, status: 0, data: { error: `network error: ${e.message}` } };
  }

  // Nginx (429/502) and proxies can return HTML/non-JSON — never let
  // res.json() reject and leave a button stuck in its disabled state.
  const text = await res.text();
  let data;
  try {
    data = text ? JSON.parse(text) : {};
  } catch {
    data = { error: text.slice(0, 300) || `HTTP ${res.status}` };
  }
  return { ok: res.ok, status: res.status, data };
}
