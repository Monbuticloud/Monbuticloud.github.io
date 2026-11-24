/// Theme mode — isDarkMode property + data-theme-mode attribute
/// data-theme-mode="Dark" | "Light" — sets CSS custom props

(function initThemeMode() {
  Object.defineProperty(document.documentElement, "isDarkMode", {
    get() {
      return this.dataset.themeMode === "Dark";
    },
    set(enabled) {
      this.dataset.themeMode = enabled ? "Dark" : "Light";
    },
    configurable: true,
    enumerable: true,
  });
})();

/// 3D Tilt — cursor-driven perspective tilt for .tilt elements

(function initTilt() {
  const TILT_MAX = 10;

  const elements = document.querySelectorAll(".tilt");

  elements.forEach((el) => {
    el.addEventListener("mousemove", (e) => {
      const rect = el.getBoundingClientRect();
      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;
      const centerX = rect.width / 2;
      const centerY = rect.height / 2;

      const rotateX = ((mouseY - centerY) / centerY) * -TILT_MAX;
      const rotateY = ((mouseX - centerX) / centerX) * TILT_MAX;

      el.style.setProperty("--tilt-x", `${rotateX}deg`);
      el.style.setProperty("--tilt-y", `${rotateY}deg`);
    });

    el.addEventListener("mouseleave", () => {
      el.style.removeProperty("--tilt-x");
      el.style.removeProperty("--tilt-y");
    });
  });
})();
