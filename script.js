let name_section = document.getElementById("name-section");
let name_section_width = name_section.offsetWidth;

for (let i = 0; i < 10; i++) {
  let div = document.createElement("div");
  div.style.backgroundColor = get_rainbow(i / 10);
  div.style.width = name_section_width / 10 + "px";
  name_section.appendChild(div);
}

function get_rainbow(t) {
  t = Math.min(1, Math.max(0, t));
  const hue = t * 300; // avoids wrapping back to red
  return `hsl(${hue}, 85%, 55%)`;
}
