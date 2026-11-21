// Composition root for index page — wires rainbow, terminal and grid.
// Kept tiny: imports modules and injects DOM refs.

import { initRainbow } from "./rainbow.js";
import { runTerminal } from "./terminal/intro.js";
import { commands } from "./terminal/commands.js";
import { createTerminalInput } from "./terminal/input.js";
import { initGrid } from "./grid/canvas.js";

function initIndexPage() {
  // Rainbow bar + text clip (no-ops if #name-section/#name-span missing)
  initRainbow();

  // Generative grid (no-ops if #grid-container missing)
  initGrid();

  // Terminal — only on index where the DOM panel exists
  const historyEl = document.getElementById("term-history");
  const typedEl = document.querySelector(".term-body .typed-text");
  const bodyEl = document.getElementById("term-body");
  const inputEl = document.getElementById("term-input");

  if (!historyEl || !typedEl || !bodyEl || !inputEl) return;

  const control = createTerminalInput({
    inputEl,
    historyEl,
    bodyEl,
    commandMap: commands,
  });

  // Kick off intro; control will be enabled when intro finishes.
  runTerminal({ historyEl, typedEl, bodyEl, control });
}

// Defer until DOM ready (works for both defer and module loading)
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initIndexPage);
} else {
  initIndexPage();
}

export { initIndexPage };
