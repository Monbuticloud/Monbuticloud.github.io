// ─── #name-section: full-width rainbow strips ───
let nameSection = document.getElementById("name-section");
let nameSectionWidth = nameSection.offsetWidth;
let numSegments = 30;

for (let i = 0; i < numSegments; i++) {
  let div = document.createElement("div");
  div.style.backgroundColor = getRainbow(i / numSegments);
  div.style.width = nameSectionWidth / numSegments + "px";
  nameSection.appendChild(div);
}

function shiftBg(container) {
  const divs = [...container.querySelectorAll("div")];
  const first = getComputedStyle(divs[0]).backgroundColor;
  for (let i = 0; i < divs.length - 1; i++) {
    divs[i].style.backgroundColor = getComputedStyle(divs[i + 1]).backgroundColor;
  }
  divs[divs.length - 1].style.backgroundColor = first;
}
setInterval(shiftBg, 75, nameSection);

// ─── #name-span: same rainbow clipped to text ───
let nameSpan = document.getElementById("name-span");
let segmentWidthPx = 50;

let stops = [];
for (let i = 0; i < numSegments; i++) {
  let color = getRainbow(i / numSegments);
  let startPx = i * segmentWidthPx;
  let endPx = (i + 1) * segmentWidthPx;
  stops.push(`${color} ${startPx}px`);
  stops.push(`${color} ${endPx}px`);
}

let totalWidth = numSegments * segmentWidthPx;
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
