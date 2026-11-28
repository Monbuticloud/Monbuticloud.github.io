// Authenticated API caller panel (FE-012).

import { $, ui, show } from "./dom.js";
import { api } from "./api.js";

export function initApiPanel() {
  ui.btnApi.addEventListener("click", onCall);
}

async function onCall() {
  const path = $("api-path").value.trim();
  ui.btnApi.disabled = true;
  show(ui.apiOut, "Calling...", "info");

  const res = await api("GET", path);

  if (res.ok) {
    show(ui.apiOut, JSON.stringify(res.data, null, 2), "success");
  } else {
    show(ui.apiOut, `${res.status}: ${res.data.error || JSON.stringify(res.data)}`, "error");
  }

  ui.btnApi.disabled = false;
}
