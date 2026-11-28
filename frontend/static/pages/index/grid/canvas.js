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
let isVisible = !document.hidden;
let isIntersecting = true;
let cachedRect = null;

const MOUSE_THROTTLE_MS = 32;
let lastMouseEventTime = 0;
let pendingPointer = null;
let mouseRafPending = false;

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

function shouldRender() {
  return isVisible && isIntersecting && document.visibilityState === "visible";
}

function scheduleFrame() {
  if (!shouldRender()) return;
  if (rafId) cancelAnimationFrame(rafId);
  rafId = requestAnimationFrame(drawFrame);
}

function pauseFrame() {
  if (rafId) {
    cancelAnimationFrame(rafId);
    rafId = 0;
  }
}

function drawFrame(timestamp) {
  rafId = 0;
  if (!shouldRender()) return;

  const { ctx, cellSize, w, h, offsetX, offsetY, canvas } = gridState;
  if (!ctx) {
    scheduleFrame();
    return;
  }

  // Advance phase with drift-free timing.
  while (timestamp - lastPhaseTime >= PHASE_INTERVAL) {
    prevPhase = diagonalPhase;
    diagonalPhase = (diagonalPhase + 1) % NUM_PHASES;
    lastPhaseTime += PHASE_INTERVAL;
  }
  const t = Math.min((timestamp - lastPhaseTime) / PHASE_INTERVAL, 1);

  // Cache the rect once per frame; invalidate on resize/scroll (FE-007).
  if (!cachedRect) cachedRect = canvas.getBoundingClientRect();
  const cr = cachedRect;
  let mx = -1;
  let my = -1;
  if (lastPointer.clientX !== -1) {
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

  scheduleFrame();
}

function debounce(fn, ms) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

/**
 * Initialize (or re-initialize) the canvas. Leaves event wiring to initGrid.
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
  cachedRect = canvas.getBoundingClientRect();
  lastPhaseTime = performance.now();
  pauseFrame();
  if (shouldRender()) rafId = requestAnimationFrame(drawFrame);

  return {
    destroy() {
      pauseFrame();
    },
  };
}

let activeTeardown = null;

/**
 * Full grid bootstrap — canvas + throttled mouse + resize/visibility gating.
 * Safe to call more than once: the previous instance is torn down first.
 * @param {object} [opts]
 * @param {string} [opts.containerId]
 * @returns {{ destroy: () => void } | null}
 */
export function initGrid({ containerId = "grid-container" } = {}) {
  if (activeTeardown) activeTeardown();

  const handle = initCanvas({ containerId });
  if (!handle) return null;

  const container = document.getElementById(containerId);

  function onMouseMove(event) {
    const now = performance.now();
    if (now - lastMouseEventTime < MOUSE_THROTTLE_MS) {
      pendingPointer = { x: event.clientX, y: event.clientY };
      if (!mouseRafPending) {
        mouseRafPending = true;
        requestAnimationFrame(() => {
          mouseRafPending = false;
          if (pendingPointer) {
            lastPointer.clientX = pendingPointer.x;
            lastPointer.clientY = pendingPointer.y;
            pendingPointer = null;
            lastMouseEventTime = performance.now();
          }
        });
      }
      return;
    }
    lastMouseEventTime = now;
    lastPointer.clientX = event.clientX;
    lastPointer.clientY = event.clientY;
  }

  function onResize() {
    cachedRect = null;
    initCanvas({ containerId });
  }

  function onScroll() {
    cachedRect = null;
  }

  function onVisibilityChange() {
    isVisible = !document.hidden;
    cachedRect = null;
    if (shouldRender()) scheduleFrame();
    else pauseFrame();
  }

  let observer = null;
  if (container && "IntersectionObserver" in window) {
    observer = new IntersectionObserver(
      (entries) => {
        isIntersecting = entries[0].isIntersecting;
        cachedRect = null;
        if (shouldRender()) scheduleFrame();
        else pauseFrame();
      },
      { threshold: 0 },
    );
    observer.observe(container);
  }

  const debouncedResize = debounce(onResize, 150);
  window.addEventListener("mousemove", onMouseMove, { passive: true });
  window.addEventListener("resize", debouncedResize);
  window.addEventListener("scroll", onScroll, { passive: true });
  document.addEventListener("visibilitychange", onVisibilityChange);

  function teardown() {
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("resize", debouncedResize);
    window.removeEventListener("scroll", onScroll);
    document.removeEventListener("visibilitychange", onVisibilityChange);
    if (observer) observer.disconnect();
    handle.destroy();
    if (activeTeardown === teardown) activeTeardown = null;
  }

  activeTeardown = teardown;
  return { destroy: teardown };
}
