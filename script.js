// ═══════════════════════════════════════════════════════════
// Deferred Font Awesome — load icons CSS after first paint
// ═══════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════
// Rainbow Name Section (restored original)
// ═══════════════════════════════════════════════════════════

const nameSection = document.getElementById("name-section");
const numSegments = 30;

for (let i = 0; i < numSegments; i++) {
  const div = document.createElement("div");
  div.style.backgroundColor = getRainbow(i / numSegments);
  div.style.flex = "1";
  nameSection.appendChild(div);
}

function shiftBg(container) {
  const divs = [...container.querySelectorAll("div")];
  const first = getComputedStyle(divs[0]).backgroundColor;
  for (let i = 0; i < divs.length - 1; i++) {
    divs[i].style.backgroundColor = getComputedStyle(
      divs[i + 1],
    ).backgroundColor;
  }
  divs[divs.length - 1].style.backgroundColor = first;
}
setInterval(shiftBg, 75, nameSection);

// Rainbow text clip
const nameSpan = document.getElementById("name-span");
const segmentWidthPx = 50;

const stops = [];
for (let i = 0; i < numSegments; i++) {
  const color = getRainbow(i / numSegments);
  const startPx = i * segmentWidthPx;
  const endPx = (i + 1) * segmentWidthPx;
  stops.push(`${color} ${startPx}px`);
  stops.push(`${color} ${endPx}px`);
}

const totalWidth = numSegments * segmentWidthPx;
nameSpan.style.backgroundImage = `linear-gradient(to right, ${stops.join(", ")})`;
nameSpan.style.backgroundSize = `${totalWidth}px 100%`;
nameSpan.style.backgroundRepeat = "repeat-x";

let offset = 0;
setInterval(() => {
  offset = (offset - segmentWidthPx) % totalWidth;
  nameSpan.style.backgroundPosition = `${offset}px 0`;
}, 75);

function getRainbow(t) {
  t = Math.min(1, Math.max(0, t));
  const hue = t * 359;
  return `hsl(${hue}, 55%, 65%)`;
}

// ═══════════════════════════════════════════════════════════
// Compact Terminal — image-split right panel
// ═══════════════════════════════════════════════════════════

const PROMPT = ">";
const TYPING_SPEED = 30;
const LINE_PAUSE = 500;
const OUTPUT_PAUSE = 220;
const INITIAL_DELAY = 1000;

const termHistory = document.getElementById("term-history");
const typedSpan = document.querySelector(".term-body .typed-text");
const termBody = document.getElementById("term-body");

const commands = [
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

function appendPowerlineHeader(time) {
  const t = time || "0ms";
  // Full path pill:  ( /Users/monbuticloud )
  const line1 = document.createElement("div");
  line1.className = "term-line pl-line pl-line-right";
  const seg1 = document.createElement("span");
  seg1.className = "pl";
  seg1.textContent = "/Users/monbuticloud";
  line1.appendChild(seg1);
  termHistory.appendChild(line1);

  // Short path pill:  ( ~ )  + timing
  const line2 = document.createElement("div");
  line2.className = "term-line pl-line";
  line2.append(document.createTextNode("  "));
  const seg2 = document.createElement("span");
  seg2.className = "pl";
  seg2.textContent = "~";
  line2.appendChild(seg2);
  const timing = document.createElement("span");
  timing.className = "pl-timing";
  timing.textContent = t;
  line2.appendChild(timing);
  termHistory.appendChild(line2);
}

function appendLine(html) {
  const div = document.createElement("div");
  div.className = "term-line";
  div.innerHTML = html;
  termHistory.appendChild(div);
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

  for (const entry of commands) {
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
}

runTerminal();
