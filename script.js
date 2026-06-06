function get_rainbow(t) {
  t = Math.min(1, Math.max(0, t));
  const hue = t * 300; // avoids wrapping back to red
  return `hsl(${hue}, 85%, 55%)`;
}
