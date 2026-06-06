let name_section = document.getElementById("name-section");
let name_section_width = name_section.offsetWidth;
let number_of_rainbow_segments = 30;

for (let i = 0; i < number_of_rainbow_segments; i++) {
  let div = document.createElement("div");
  div.style.backgroundColor = get_rainbow(i / number_of_rainbow_segments);
  div.style.width = name_section_width / number_of_rainbow_segments + "px";
  name_section.appendChild(div);
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
setInterval(shiftBg, 75, name_section);

function get_rainbow(t) {
  t = Math.min(1, Math.max(0, t));
  const hue = t * 359;
  return `hsl(${hue}, 55%, 65%)`;
}
