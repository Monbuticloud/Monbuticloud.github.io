/// Deferred Font Awesome — load icons CSS after first paint

(function loadFontAwesome() {
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href =
    "https://cdnjs.cloudflare.com/ajax/libs/font-awesome/7.0.1/css/all.min.css";
  link.integrity =
    "sha512-2SwdPD6INVrV/lHTZbO2nodKhrnDdJK9/kg2XD1r9uGqPo1cUbujc+IYdlYdEErWNu69gVcYgdxlmVmzTWnetw==";
  link.crossOrigin = "anonymous";
  link.referrerPolicy = "no-referrer";
  document.head.appendChild(link);
})();

/// Rainbow Name Section

function getRainbow(ratio) {
  ratio = Math.min(1, Math.max(0, ratio));
  const hue = ratio * 359;
  return `hsl(${hue}, 55%, 65%)`;
}

const nameSection = document.getElementById("name-section");
const numSegments = 30;

for (let i = 0; i < numSegments; i++) {
  const segment = document.createElement("div");
  segment.style.backgroundColor = getRainbow(i / numSegments);
  segment.style.flex = "1";
  nameSection.appendChild(segment);
}

function shiftBg(container) {
  const divs = [...container.querySelectorAll("div")];
  const first = divs[0].style.backgroundColor;
  for (let i = 0; i < divs.length - 1; i++) {
    divs[i].style.backgroundColor = divs[i + 1].style.backgroundColor;
  }
  divs[divs.length - 1].style.backgroundColor = first;
}
setInterval(shiftBg, 75, nameSection);

// Rainbow text clip
const nameSpan = document.getElementById("name-span");
const segmentWidthPx = 50;

function buildRainbowStops(segmentCount, widthPx) {
  const stops = [];
  for (let i = 0; i < segmentCount; i++) {
    const color = getRainbow(i / segmentCount);
    const startPx = i * widthPx;
    const endPx = (i + 1) * widthPx;
    stops.push(`${color} ${startPx}px`);
    stops.push(`${color} ${endPx}px`);
  }
  return stops;
}

const stops = buildRainbowStops(numSegments, segmentWidthPx);
const totalWidth = numSegments * segmentWidthPx;
nameSpan.style.backgroundImage = `linear-gradient(to right, ${stops.join(", ")})`;
nameSpan.style.backgroundSize = `${totalWidth}px 100%`;
nameSpan.style.backgroundRepeat = "repeat-x";

let offset = 0;
setInterval(() => {
  offset = (offset - segmentWidthPx) % totalWidth;
  nameSpan.style.backgroundPosition = `${offset}px 0`;
}, 75);

/// Compact Terminal — image-split right panel

const PROMPT = ">";
const TYPING_SPEED = 30;
const LINE_PAUSE = 1000;
const OUTPUT_PAUSE = 220;
const INITIAL_DELAY = 2000;

const termHistory = document.getElementById("term-history");
const typedSpan = document.querySelector(".term-body .typed-text");
const termBody = document.getElementById("term-body");

const introCommands = [
  {
    cmd: "whoami",
    output: ["monbuticloud"],
    time: "0ms",
  },
  {
    cmd: "ls",
    output: ["specialties", "projects", "message.txt"],
    time: "2ms",
  },
  {
    cmd: "ls ./specialties",
    output: [
      "  low-level programming.md",
      "  backend development.md",
      "  algorithmic optimization.md",
      "  agentic programming.md",
    ],
    time: "4ms",
  },
  {
    cmd: "cd ./projects",
    output: [
      "ls: cannot open directory 'dir_name': Permission denied",
      "Maybe click on the projects link?",
    ],
    time: "1ms",
  },
  {
    cmd: "cat message.txt",
    output: [
      "Hi, I'm Mon! I can do fullstack development, but I specialize in backend development.",
    ],
    time: "1ms",
  },
  {
    cmd: "",
    output: null,
    time: "0ms",
  },
];

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function appendPowerlineHeader(executionTime) {
  const t = executionTime || "0ms";
  // Full path pill:  ( /Users/monbuticloud )
  const fullPathLine = document.createElement("div");
  fullPathLine.className = "term-line pl-line pl-line-right";
  const fullPathSegment = document.createElement("span");
  fullPathSegment.className = "pl";
  fullPathSegment.textContent = "/Users/monbuticloud";
  fullPathLine.appendChild(fullPathSegment);
  termHistory.appendChild(fullPathLine);

  // Short path pill:  ( ~ )  + timing
  const shortPathLine = document.createElement("div");
  shortPathLine.className = "term-line pl-line";
  shortPathLine.append(document.createTextNode("  "));
  const shortPathSegment = document.createElement("span");
  shortPathSegment.className = "pl";
  shortPathSegment.textContent = "~";
  shortPathLine.appendChild(shortPathSegment);
  const timingSpan = document.createElement("span");
  timingSpan.className = "pl-timing";
  timingSpan.textContent = t;
  shortPathLine.appendChild(timingSpan);
  termHistory.appendChild(shortPathLine);
}

function appendLine(htmlContent) {
  const lineElement = document.createElement("div");
  lineElement.className = "term-line";
  lineElement.innerHTML = htmlContent;
  termHistory.appendChild(lineElement);
}

function freezePromptLine() {
  const typed = typedSpan.textContent;
  appendLine(`<span class="prompt">${PROMPT}</span> ${typed}`);
  typedSpan.textContent = "";
}

async function typeText(text) {
  for (const char of text) {
    typedSpan.textContent += char;
    await sleep(TYPING_SPEED);
  }
}

async function runTerminal() {
  await sleep(INITIAL_DELAY);

  for (const entry of introCommands) {
    appendPowerlineHeader(entry.time);
    await typeText(entry.cmd);
    await sleep(LINE_PAUSE);

    if (entry.output) {
      freezePromptLine();

      const outputs = Array.isArray(entry.output)
        ? entry.output
        : [entry.output];
      for (const line of outputs) {
        await sleep(OUTPUT_PAUSE);
        appendLine(`<span class="output-line">${line}</span>`);
      }
    } else {
      freezePromptLine();
    }

    termBody.scrollTop = termBody.scrollHeight;
  }

  // Intro done — unlock input
  termControl.enable();
}

runTerminal();

/// ───────── Terminal Commands ─────────
/// Maps keyword → { handler, description }.
/// Add new commands here.

const commands = {
  echo: {
    description: "Echo back what you type",
    handler: (args, output) => output(`  ${args}`),
  },
  help: {
    description: "Show available commands",
    handler: (_args, output) => {
      output("  Available commands:");
      for (const [name, cmd] of Object.entries(commands)) {
        output(`    ${name}  —  ${cmd.description}`);
      }
    },
  },
  cat: {
    description: "Concatenate files",
    handler: (args, output) => {
      if (args === "message.txt") {
        output("  Hi, I'm Mon! I can do fullstack development, but I specialize in backend development.");
      } else if (args.startsWith("specialties/")) {
        output("  cat: " + args + ": Permission denied");
      } else {
        output("  cat: " + (args || "(no file)") + ": No such file");
      }
    },
  },
  cd: {
    description: "Change directory",
    handler: (_args, output) => output("  You cannot change directory"),
  },
  ssh: {
    description: "SSH into a remote host",
    handler: (args, output) => {
      const targets = {
        "monbuticloud@projects.local": "projects.html",
        "monbuticloud@contact.local": "contact.html",
      };
      const page = targets[args.trim()];
      if (page) {
        output(`  Connecting to ${args.trim()}...`);
        setTimeout(() => { window.location.href = page; }, 600);
      } else {
        output(`  ssh: connect to host ${args.split("@").pop() || args} port 22: Connection refused`);
      }
    },
  },
  clear: {
    description: "Clear the terminal",
    handler: (_args, output) => {
      const history = document.getElementById("term-history");
      history.innerHTML = "";
    },
  },
  // TODO: add more commands here
};

/// ───────── Terminal Interactive Input ─────────
/// Injected dependencies — no global coupling.

function createTerminalInput(inject) {
  const { inputEl, historyEl, bodyEl, commandMap } = inject;

  function appendOutputLine(text) {
    const line = document.createElement("div");
    line.className = "term-line";
    line.innerHTML = `<span class="output-line">${text}</span>`;
    historyEl.appendChild(line);
    bodyEl.scrollTop = bodyEl.scrollHeight;
  }

  let lastCommandTime = performance.now();

  function appendCommandLine(text) {
    const now = performance.now();
    const elapsed = Math.round(now - lastCommandTime);
    lastCommandTime = now;
    const timing = elapsed < 1 ? "0ms" : elapsed + "ms";
    // Powerline header — matches intro styling
    appendPowerlineHeader(timing);
    const line = document.createElement("div");
    line.className = "term-line";
    line.innerHTML = `<span class="prompt">&gt;</span> ${text}`;
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

// Wire it up with injected refs
const termControl = createTerminalInput({
  inputEl: document.getElementById("term-input"),
  historyEl: termHistory,
  bodyEl: termBody,
  commandMap: commands,
});

/// Square Grid — canvas-based generative pattern

const GRID_WIDTH = 40;
const GRID_HEIGHT = 30;
const DISTORT_RADIUS = 250;
const DISTORT_RADIUS_SQ = DISTORT_RADIUS * DISTORT_RADIUS;
const MAX_SHIFT = 20;
const TOTAL_CELLS = GRID_WIDTH * GRID_HEIGHT;
const NUM_PHASES = GRID_WIDTH + GRID_HEIGHT - 1;
const lightnessCache = [];
const PHASE_INTERVAL = 200; // ms

const gridState = { canvas: null, ctx: null, cellSize: 0, w: 0, h: 0, offsetX: 0, offsetY: 0 };
const lastPointer = { clientX: -1, clientY: -1 };
let diagonalPhase = 0;
let prevPhase = 0;
let lastPhaseTime = 0;

function buildLightnessCache() {
  for (let phase = 0; phase < NUM_PHASES; phase++) {
    const arr = new Float64Array(TOTAL_CELLS);
    for (let i = 0; i < TOTAL_CELLS; i++) {
      arr[i] = getDiagonalLightness(Math.floor(i / GRID_WIDTH), i % GRID_WIDTH, phase);
    }
    lightnessCache.push(arr);
  }
}

function getDiagonalLightness(row, col, phase = 0) {
  const maxDiagonal = GRID_WIDTH + GRID_HEIGHT - 2;
  const progress = ((row + col + phase) % (maxDiagonal + 1)) / maxDiagonal;
  const triangleWave = progress < 0.5 ? progress * 2 : 2 - progress * 2;
  return 25 + triangleWave * 50;
}

function initCanvas() {
  const container = document.getElementById("grid-container");
  if (!container) return;

  container.innerHTML = "";
  container.style.contain = "style layout";

  const canvas = document.createElement("canvas");
  canvas.id = "grid-canvas";
  container.appendChild(canvas);

  const rect = container.getBoundingClientRect();
  const cellSize = rect.width / GRID_WIDTH;
  const gridH = cellSize * GRID_HEIGHT;

  canvas.width = rect.width * devicePixelRatio;
  canvas.height = gridH * devicePixelRatio;
  canvas.style.width = `${rect.width}px`;
  canvas.style.height = `${gridH}px`;
  const ctx = canvas.getContext("2d");
  ctx.scale(devicePixelRatio, devicePixelRatio);

  gridState.canvas = canvas;
  gridState.ctx = ctx;
  gridState.cellSize = cellSize;
  gridState.w = rect.width;
  gridState.h = gridH;
  gridState.offsetX = 0;
  gridState.offsetY = 0;

  if (lightnessCache.length === 0) buildLightnessCache();
  lastPhaseTime = performance.now();
  cancelAnimationFrame(rafId);
  rafId = requestAnimationFrame(drawFrame);
}

function drawFrame(timestamp) {
  const { ctx, cellSize, w, h, offsetX, offsetY, canvas } = gridState;
  if (!ctx) { rafId = requestAnimationFrame(drawFrame); return; }

  // Advance phase with drift-free timing
  while (timestamp - lastPhaseTime >= PHASE_INTERVAL) {
    prevPhase = diagonalPhase;
    diagonalPhase = (diagonalPhase + 1) % NUM_PHASES;
    lastPhaseTime += PHASE_INTERVAL;
  }
  const t = Math.min((timestamp - lastPhaseTime) / PHASE_INTERVAL, 1);

  // Recalculate mouse relative to canvas (handles scroll/resize)
  let mx = -1, my = -1;
  if (lastPointer.clientX !== -1) {
    const cr = canvas.getBoundingClientRect();
    mx = lastPointer.clientX - cr.left;
    my = lastPointer.clientY - cr.top;
  }

  const prevCache = lightnessCache[prevPhase];
  const currCache = lightnessCache[diagonalPhase];

  ctx.clearRect(0, 0, w, h);

  for (let row = 0; row < GRID_HEIGHT; row++) {
    for (let col = 0; col < GRID_WIDTH; col++) {
      const idx = row * GRID_WIDTH + col;

      // Interpolated lightness
      const lightness = prevCache[idx] + (currCache[idx] - prevCache[idx]) * t;
      ctx.fillStyle = `hsl(0, 0%, ${lightness}%)`;

      // Position with optional mouse distortion
      let x = offsetX + col * cellSize;
      let y = offsetY + row * cellSize;

      if (mx !== -1) {
        const cx = x + cellSize / 2;
        const cy = y + cellSize / 2;
        const dx = cx - mx;
        const dy = cy - my;
        const distSq = dx * dx + dy * dy;

        if (distSq < DISTORT_RADIUS_SQ && distSq > 0) {
          const dist = Math.sqrt(distSq);
          const ratio = dist / DISTORT_RADIUS;
          const falloff = 1 - ratio * ratio * ratio;
          const shift = falloff * MAX_SHIFT;
          x += (dx / dist) * shift;
          y += (dy / dist) * shift;
        }
      }

      ctx.fillRect(x, y, cellSize, cellSize);
    }
  }

  rafId = requestAnimationFrame(drawFrame);
}

// Events — track mouse anywhere near the grid
window.addEventListener("mousemove", (e) => {
  lastPointer.clientX = e.clientX;
  lastPointer.clientY = e.clientY;
});

// Init + resize
function debounce(fn, ms) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

let rafId = 0;
initCanvas();
window.addEventListener("resize", debounce(initCanvas, 150));

/// ─── Dither Hero Image ───────────────────────────────────
(function ditherHero() {
  const canvas = document.getElementById("dither-canvas");
  if (!canvas) return;

  const palette = [0, 85, 170, 255]; // 4 grayscale levels

  function floydSteinberg(data, w, h) {
    const d = new Uint8ClampedArray(data);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const i = (y * w + x) * 4;
        const old = d[i];
        let nearest = 0, minDist = Infinity;
        for (let k = 0; k < palette.length; k++) {
          const dist = Math.abs(old - palette[k]);
          if (dist < minDist) { minDist = dist; nearest = palette[k]; }
        }
        const err = old - nearest;
        d[i] = d[i + 1] = d[i + 2] = nearest;
        // distribute error (Floyd-Steinberg weights)
        if (x + 1 < w) {
          const r = i + 4;
          d[r] += err * 7 / 16; d[r + 1] = d[r + 2] = d[r];
        }
        if (y + 1 < h) {
          if (x > 0) {
            const bl = i + w * 4 - 4;
            d[bl] += err * 3 / 16; d[bl + 1] = d[bl + 2] = d[bl];
          }
          const b = i + w * 4;
          d[b] += err * 5 / 16; d[b + 1] = d[b + 2] = d[b];
          if (x + 1 < w) {
            const br = i + w * 4 + 4;
            d[br] += err * 1 / 16; d[br + 1] = d[br + 2] = d[br];
          }
        }
      }
    }
    return new ImageData(d, w, h);
  }

  function render() {
    const img = new Image();
    img.onload = function () {
      const srcW = img.naturalWidth, srcH = img.naturalHeight;
      if (!srcW || !srcH) return;
      // dither at native res
      const srcC = document.createElement("canvas");
      srcC.width = srcW; srcC.height = srcH;
      const srcCtx = srcC.getContext("2d");
      srcCtx.imageSmoothingEnabled = false;
      srcCtx.drawImage(img, 0, 0);
      try {
        const imageData = srcCtx.getImageData(0, 0, srcW, srcH);
        const dithered = floydSteinberg(imageData.data, srcW, srcH);
        srcCtx.putImageData(dithered, 0, 0);
      } catch (_) {
        // canvas tainted (file:// protocol) — skip dither, show raw
      }
      // scale to display size
      const rect = canvas.parentElement.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      const dpr = window.devicePixelRatio || 1;
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      canvas.style.width = rect.width + "px";
      canvas.style.height = rect.height + "px";
      const ctx = canvas.getContext("2d");
      ctx.scale(dpr, dpr);
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(srcC, 0, 0, rect.width, rect.height);
    };
    img.onerror = function () {
      // fallback: bypass dither, draw raw image
      const rect = canvas.parentElement.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      const dpr = window.devicePixelRatio || 1;
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      canvas.style.width = rect.width + "px";
      canvas.style.height = rect.height + "px";
      const ctx = canvas.getContext("2d");
      ctx.scale(dpr, dpr);
      ctx.imageSmoothingEnabled = false;
      // draw the broken-image icon alternative
      ctx.fillStyle = "#151525";
      ctx.fillRect(0, 0, rect.width, rect.height);
      ctx.fillStyle = "#555";
      ctx.font = Math.min(rect.width, rect.height) * 0.1 + "px monospace";
      ctx.textAlign = "center";
      ctx.fillText("image load failed", rect.width / 2, rect.height / 2);
    };
    img.src = "assets/images/optim_main_4_gray.avif";
  }

  render();
  window.addEventListener("resize", debounce(render, 200));
})();
