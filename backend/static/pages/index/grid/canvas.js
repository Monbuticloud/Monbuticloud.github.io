// Square Grid — canvas-based generative pattern

export const GRID_WIDTH = 40;
export const GRID_HEIGHT = 30;
export const DISTORT_RADIUS = 250;
export const DISTORT_RADIUS_SQ = DISTORT_RADIUS * DISTORT_RADIUS;
export const MAX_SHIFT = 20;
export const TOTAL_CELLS = GRID_WIDTH * GRID_HEIGHT;
export const NUM_PHASES = GRID_WIDTH + GRID_HEIGHT - 1;
export const PHASE_INTERVAL = 200; // ms

const lightnessCache = [];
const gridState = { canvas: null, ctx: null, cellSize: 0, w: 0, h: 0, offsetX: 0, offsetY: 0 };
const lastPointer = { clientX: -1, clientY: -1 };
let diagonalPhase = 0;
let prevPhase = 0;
let lastPhaseTime = 0;
let rafId = 0;

export function getDiagonalLightness(row, col, phase = 0) {
  const maxDiagonal = GRID_WIDTH + GRID_HEIGHT - 2;
  const progress = ((row + col + phase) % (maxDiagonal + 1)) / maxDiagonal;
  const triangleWave = progress < 0.5 ? progress * 2 : 2 - progress * 2;
  return 25 + triangleWave * 50;
}

export function buildLightnessCache() {
  if (lightnessCache.length !== 0) return;
  for (let phase = 0; phase < NUM_PHASES; phase++) {
    const arr = new Float64Array(TOTAL_CELLS);
    for (let i = 0; i < TOTAL_CELLS; i++) {
      arr[i] = getDiagonalLightness(Math.floor(i / GRID_WIDTH), i % GRID_WIDTH, phase);
    }
    lightnessCache.push(arr);
  }
}

export function getCache() {
  return lightnessCache;
}

export function getGridState() {
  return gridState;
}

function drawFrame(timestamp) {
  const { ctx, cellSize, w, h, offsetX, offsetY, canvas } = gridState;
  if (!ctx) {
    rafId = requestAnimationFrame(drawFrame);
    return;
  }

  while (timestamp - lastPhaseTime >= PHASE_INTERVAL) {
    prevPhase = diagonalPhase;
    diagonalPhase = (diagonalPhase + 1) % NUM_PHASES;
    lastPhaseTime += PHASE_INTERVAL;
  }
  const t = Math.min((timestamp - lastPhaseTime) / PHASE_INTERVAL, 1);

  let mx = -1;
  let my = -1;
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
      const lightness = prevCache[idx] + (currCache[idx] - prevCache[idx]) * t;
      ctx.fillStyle = `hsl(0, 0%, ${lightness}%)`;

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

function debounce(fn, ms) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

/**
 * Initialize canvas and animation loop.
 * @param {object} [opts]
 * @param {string} [opts.containerId="grid-container"]
 * @returns {{ destroy: () => void } | null}
 */
export function initCanvas({ containerId = "grid-container" } = {}) {
  const container = document.getElementById(containerId);
  if (!container) return null;

  container.replaceChildren();
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

  return {
    destroy() {
      cancelAnimationFrame(rafId);
    },
  };
}

let resizeHandler = null;
let mouseHandler = null;

/**
 * Full grid bootstrap — canvas + mouse + resize.
 * Safe to call once per page load.
 * @param {object} [opts]
 * @param {string} [opts.containerId]
 * @returns {{ destroy: () => void } | null}
 */
export function initGrid({ containerId = "grid-container" } = {}) {
  const handle = initCanvas({ containerId });
  if (!handle) return null;

  mouseHandler = (e) => {
    lastPointer.clientX = e.clientX;
    lastPointer.clientY = e.clientY;
  };
  window.addEventListener("mousemove", mouseHandler);

  const debounced = debounce(() => initCanvas({ containerId }), 150);
  resizeHandler = debounced;
  window.addEventListener("resize", resizeHandler);

  return {
    destroy() {
      handle.destroy();
      if (mouseHandler) window.removeEventListener("mousemove", mouseHandler);
      if (resizeHandler) window.removeEventListener("resize", resizeHandler);
    },
  };
}
