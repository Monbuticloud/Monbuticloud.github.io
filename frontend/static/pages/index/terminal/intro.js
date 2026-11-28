// Compact Terminal — typed intro sequence
// Depends on DOM elements injected at call time (no global coupling).

export const PROMPT = ">";
export const TYPING_SPEED = 30;
export const LINE_PAUSE = 1000;
export const OUTPUT_PAUSE = 220;
export const INITIAL_DELAY = 2000;

export const introCommands = [
  { cmd: "whoami", output: ["monbuticloud"], time: "0ms" },
  { cmd: "ls", output: ["specialties", "projects", "message.txt"], time: "2ms" },
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
    output: ["Hi, I'm Mon! I can do fullstack development, but I specialize in backend development."],
    time: "1ms",
  },
  { cmd: "", output: null, time: "0ms" },
];

export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Append powerline header (two lines) into history.
 * @param {HTMLElement} historyEl
 * @param {string} executionTime
 */
export function appendPowerlineHeader(historyEl, executionTime) {
  const t = executionTime || "0ms";

  const fullPathLine = document.createElement("div");
  fullPathLine.className = "term-line pl-line pl-line-right";
  const fullPathSegment = document.createElement("span");
  fullPathSegment.className = "pl";
  fullPathSegment.textContent = "/Users/monbuticloud";
  fullPathLine.appendChild(fullPathSegment);
  historyEl.appendChild(fullPathLine);

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
  historyEl.appendChild(shortPathLine);
}

/**
 * Append an output line. Uses textContent so command output cannot inject
 * HTML (FE-001).
 * @param {HTMLElement} historyEl
 * @param {string} text
 */
export function appendOutputLine(historyEl, text) {
  const lineElement = document.createElement("div");
  lineElement.className = "term-line";
  const span = document.createElement("span");
  span.className = "output-line";
  span.textContent = text;
  lineElement.appendChild(span);
  historyEl.appendChild(lineElement);
}

/**
 * Freeze the live typed line into history.
 * @param {HTMLElement} historyEl
 * @param {HTMLElement} typedEl
 */
export function freezePromptLine(historyEl, typedEl) {
  const typed = typedEl.textContent;
  const lineElement = document.createElement("div");
  lineElement.className = "term-line";
  const promptSpan = document.createElement("span");
  promptSpan.className = "prompt";
  promptSpan.textContent = PROMPT;
  lineElement.appendChild(promptSpan);
  lineElement.appendChild(document.createTextNode(` ${typed}`));
  historyEl.appendChild(lineElement);
  typedEl.textContent = "";
}

/**
 * Type text char-by-char into typedEl.
 * @param {HTMLElement} typedEl
 * @param {string} text
 * @param {number} [speed=TYPING_SPEED]
 */
export async function typeText(typedEl, text, speed = TYPING_SPEED) {
  for (const char of text) {
    typedEl.textContent += char;
    await sleep(speed);
  }
}

/**
 * Run the intro sequence, then enable interactive input.
 * @param {object} deps
 * @param {HTMLElement} deps.historyEl
 * @param {HTMLElement} deps.typedEl
 * @param {HTMLElement} deps.bodyEl
 * @param {{ enable: () => void }} deps.control - input control
 * @param {object} [deps.config]
 */
export async function runTerminal({ historyEl, typedEl, bodyEl, control, config = {} }) {
  const {
    typingSpeed = TYPING_SPEED,
    linePause = LINE_PAUSE,
    outputPause = OUTPUT_PAUSE,
    initialDelay = INITIAL_DELAY,
    commands: cmds = introCommands,
  } = config;

  await sleep(initialDelay);

  for (const entry of cmds) {
    appendPowerlineHeader(historyEl, entry.time);
    await typeText(typedEl, entry.cmd, typingSpeed);
    await sleep(linePause);

    if (entry.output) {
      freezePromptLine(historyEl, typedEl);
      const outputs = Array.isArray(entry.output) ? entry.output : [entry.output];
      for (const line of outputs) {
        await sleep(outputPause);
        appendOutputLine(historyEl, line);
      }
    } else {
      freezePromptLine(historyEl, typedEl);
    }

    bodyEl.scrollTop = bodyEl.scrollHeight;
  }

  if (control && typeof control.enable === "function") {
    control.enable();
  }
}
