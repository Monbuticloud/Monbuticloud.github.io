// ═══════════════════════════════════════════════════════════
// 3D Tilt — cursor-driven perspective tilt for .tilt elements
// ═══════════════════════════════════════════════════════════

(function initTilt() {
  const TILT_MAX = 10; // max rotation in degrees

  const elements = document.querySelectorAll(".tilt");

  elements.forEach((el) => {
    el.addEventListener("mousemove", (e) => {
      const rect = el.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const centerX = rect.width / 2;
      const centerY = rect.height / 2;

      const rotateX = ((y - centerY) / centerY) * -TILT_MAX;
      const rotateY = ((x - centerX) / centerX) * TILT_MAX;

      el.style.setProperty("--tilt-x", `${rotateX}deg`);
      el.style.setProperty("--tilt-y", `${rotateY}deg`);
    });

    el.addEventListener("mouseleave", () => {
      el.style.removeProperty("--tilt-x");
      el.style.removeProperty("--tilt-y");
    });
  });
})();
