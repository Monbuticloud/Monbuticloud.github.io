// Terminal Interactive Input — injected dependencies, no global coupling.

import { appendPowerlineHeader } from "./intro.js";

/**
 * Create the interactive terminal input controller.
 * @param {object} inject
 * @param {HTMLInputElement} inject.inputEl
 * @param {HTMLElement} inject.historyEl
 * @param {HTMLElement} inject.bodyEl
 * @param {Record<string, { description: string, handler: (args: string, output: (t: string) => void) => void }>} inject.commandMap
 * @returns {{ enable: () => void, disable: () => void }}
 */
export function createTerminalInput({ inputEl, historyEl, bodyEl, commandMap }) {
  function appendOutputLine(text) {
    const line = document.createElement("div");
    line.className = "term-line";
    const span = document.createElement("span");
    span.className = "output-line";
    span.textContent = text;
    line.appendChild(span);
    historyEl.appendChild(line);
    bodyEl.scrollTop = bodyEl.scrollHeight;
  }

  let lastCommandTime = performance.now();

  function appendCommandLine(text) {
    const now = performance.now();
    const elapsed = Math.round(now - lastCommandTime);
    lastCommandTime = now;
    const timing = elapsed < 1 ? "0ms" : `${elapsed}ms`;
    appendPowerlineHeader(historyEl, timing);
    const line = document.createElement("div");
    line.className = "term-line";
    const promptSpan = document.createElement("span");
    promptSpan.className = "prompt";
    promptSpan.textContent = ">";
    line.appendChild(promptSpan);
    line.appendChild(document.createTextNode(` ${text}`));
    historyEl.appendChild(line);
    bodyEl.scrollTop = bodyEl.scrollHeight;
  }

  function handleCommand(raw) {
    const trimmed = raw.trim();
    if (!trimmed) return;

    appendCommandLine(trimmed);

    const [keyword, ...rest] = trimmed.split(/\s+/);
    const entry = commandMap[keyword.toLowerCase()];

    if (entry) {
      entry.handler(rest.join(" "), appendOutputLine);
    } else {
      appendOutputLine(`zsh: command not found: ${keyword}`);
    }
  }

  inputEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleCommand(inputEl.value);
      inputEl.value = "";
    }
  });

  return {
    enable() {
      inputEl.disabled = false;
      inputEl.placeholder = "type a command…";
      inputEl.focus();
    },
    disable() {
      inputEl.disabled = true;
      inputEl.placeholder = "waiting for intro…";
    },
  };
}
