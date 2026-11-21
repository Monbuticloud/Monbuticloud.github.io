// Playwright config for the frontend smoke tests (FE-011).
//
// CJS on purpose: backend/package.json has no "type": "module", so a .js config
// using `import` would not load. The static server is an .mjs for the same reason.
const { defineConfig, devices } = require("@playwright/test");

const PORT = Number(process.env.PORT ?? 4173);

module.exports = defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: "on-first-retry",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "node e2e/serve.mjs",
    url: `http://localhost:${PORT}/index.html`,
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});