// Rainbow Name Section — bar + text-clip gradient animation

/**
 * Convert a 0..1 ratio to a rainbow HSL color.
 * @param {number} ratio - Normalized position 0..1.
 * @returns {string} CSS hsl() string.
 */
export function getRainbow(ratio) {
  ratio = Math.min(1, Math.max(0, ratio));
  const hue = ratio * 359;
  return `hsl(${hue}, 55%, 65%)`;
}

/**
 * Build hard-stop gradient stops for a segmented rainbow.
 * @param {number} segmentCount
 * @param {number} widthPx - Width of a single segment in px.
 * @returns {string[]} Array of "<color> <px>px" stops.
 */
export function buildRainbowStops(segmentCount, widthPx) {
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

/**
 * Rotate background colors of children one step forward.
 * @param {Element} container - Parent whose div children hold colors.
 */
export function shiftBg(container) {
  const divs = [...container.querySelectorAll("div")];
  if (divs.length === 0) return;
  const first = divs[0].style.backgroundColor;
  for (let i = 0; i < divs.length - 1; i++) {
    divs[i].style.backgroundColor = divs[i + 1].style.backgroundColor;
  }
  divs[divs.length - 1].style.backgroundColor = first;
}

/**
 * Initialize rainbow bar + animated text clip.
 * Queries DOM by id; no-ops if elements are missing (page reuse).
 *
 * @param {object} [opts]
 * @param {string} [opts.nameSectionId="name-section"]
 * @param {string} [opts.nameSpanId="name-span"]
 * @param {number} [opts.numSegments=30]
 * @param {number} [opts.segmentWidthPx=50]
 * @param {number} [opts.intervalMs=75]
 * @returns {{ destroy: () => void } | null} cleanup handle or null if missing.
 */
export function initRainbow({
  nameSectionId = "name-section",
  nameSpanId = "name-span",
  numSegments = 30,
  segmentWidthPx = 50,
  intervalMs = 75,
} = {}) {
  const nameSection = document.getElementById(nameSectionId);
  const nameSpan = document.getElementById(nameSpanId);
  if (!nameSection || !nameSpan) return null;

  // Bar: create segments
  for (let i = 0; i < numSegments; i++) {
    const segment = document.createElement("div");
    segment.style.backgroundColor = getRainbow(i / numSegments);
    segment.style.flex = "1";
    nameSection.appendChild(segment);
  }

  // Text clip gradient
  const stops = buildRainbowStops(numSegments, segmentWidthPx);
  const totalWidth = numSegments * segmentWidthPx;
  nameSpan.style.backgroundImage = `linear-gradient(to right, ${stops.join(", ")})`;
  nameSpan.style.backgroundSize = `${totalWidth}px 100%`;
  nameSpan.style.backgroundRepeat = "repeat-x";

  let offset = 0;
  let barTimer = null;
  let textTimer = null;
  let isVisible = !document.hidden;
  let isIntersecting = true;

  function tickBar() {
    shiftBg(nameSection);
  }

  function tickText() {
    offset = (offset - segmentWidthPx) % totalWidth;
    nameSpan.style.backgroundPosition = `${offset}px 0`;
  }

  function start() {
    if (barTimer === null) barTimer = setInterval(tickBar, intervalMs);
    if (textTimer === null) textTimer = setInterval(tickText, intervalMs);
  }

  function stop() {
    if (barTimer !== null) {
      clearInterval(barTimer);
      barTimer = null;
    }
    if (textTimer !== null) {
      clearInterval(textTimer);
      textTimer = null;
    }
  }

  // FE-013: only animate while the hero is visible and on-screen.
  function sync() {
    if (isVisible && isIntersecting) start();
    else stop();
  }

  function onVisibilityChange() {
    isVisible = !document.hidden;
    sync();
  }

  document.addEventListener("visibilitychange", onVisibilityChange);

  let observer = null;
  if ("IntersectionObserver" in window) {
    observer = new IntersectionObserver(
      (entries) => {
        isIntersecting = entries[0].isIntersecting;
        sync();
      },
      { threshold: 0 },
    );
    observer.observe(nameSection);
  }

  sync();

  return {
    destroy() {
      stop();
      document.removeEventListener("visibilitychange", onVisibilityChange);
      if (observer) observer.disconnect();
    },
  };
}
