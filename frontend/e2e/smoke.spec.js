// Frontend smoke tests (FE-011).
//
// These exist because FE-001..FE-010 accumulated unnoticed: nothing ever loaded
// the pages. Each test asserts what a visitor would notice breaking, not markup
// details — an uncaught exception, a page that renders nothing, a terminal that
// never accepts input.
//
// Two deliberate non-goals:
//   - No visual assertions. Screenshots in CI rot and get rubber-stamped.
//   - `pageerror` (uncaught JS exception) is the failure signal, not
//     `console.error`: the auth page deliberately 404s /falcon_wasm.js here
//     (that bundle is built by crates/falcon-wasm/build.sh and served by the
//     Rust server), so a console error is expected in this harness.
const { test, expect } = require("@playwright/test");

/** Collect uncaught exceptions for a page; assert nothing threw. */
function trackPageErrors(page) {
  const errors = [];
  page.on("pageerror", (err) => errors.push(err.message));
  return errors;
}

test.describe("index", () => {
  test("renders and boots the terminal", async ({ page }) => {
    const errors = trackPageErrors(page);

    await page.goto("/index.html");

    await expect(page).toHaveTitle(/Monbuticloud/);
    await expect(page.locator("h1")).toBeVisible();

    // The intro types itself out before enabling input: 2s initial delay, then
    // ~30ms/char per command plus a 1s pause between lines. Generous timeout.
    const input = page.locator("#term-input");
    await expect(input).toBeEnabled({ timeout: 60_000 });
    // Pattern, not an exact string: input.js sets a Unicode ellipsis here
    // while the markup ships ASCII dots, and the difference is invisible.
    await expect(input).toHaveAttribute("placeholder", /^type a command/);

    expect(errors).toEqual([]);
  });

  test("answers 'help' with the command list", async ({ page }) => {
    await page.goto("/index.html");
    const input = page.locator("#term-input");
    await expect(input).toBeEnabled({ timeout: 60_000 });

    await input.fill("help");
    await input.press("Enter");

    await expect(page.locator("#term-history")).toContainText("Available commands:");
  });

  test("echo renders markup as literal text (FE-001 regression)", async ({ page }) => {
    await page.goto("/index.html");
    const input = page.locator("#term-input");
    await expect(input).toBeEnabled({ timeout: 60_000 });

    const payload = "<img src=x onerror=window.__pwned=1>";
    await input.fill(`echo ${payload}`);
    await input.press("Enter");

    // Rendered as text...
    await expect(page.locator("#term-history")).toContainText(payload);
    // ...and not parsed into an element. This is the assertion that would have
    // caught FE-001: appendOutputLine must stay on textContent, never innerHTML.
    await expect(page.locator("#term-history img")).toHaveCount(0);
    expect(await page.evaluate(() => window.__pwned)).toBeUndefined();
  });
});

test.describe("contact", () => {
  test("renders", async ({ page }) => {
    const errors = trackPageErrors(page);

    await page.goto("/contact.html");

    await expect(page).toHaveTitle(/Contact/);
    // These pages carry no <h1> — the section heading is an <h2>. Asserting the
    // real heading keeps the test honest about what the page actually ships.
    await expect(page.locator("main h2").first()).toContainText("Get in Touch");
    expect(errors).toEqual([]);
  });
});

test.describe("projects", () => {
  test("renders", async ({ page }) => {
    const errors = trackPageErrors(page);

    await page.goto("/projects.html");

    await expect(page).toHaveTitle(/Projects/);
    await expect(page.locator("main h2").first()).toContainText("Projects");
    expect(errors).toEqual([]);
  });
});

test.describe("auth", () => {
  test("boots its ES modules and wires the controls", async ({ page }) => {
    const errors = trackPageErrors(page);

    await page.goto("/auth.html");

    await expect(page).toHaveTitle(/Auth/);

    // These ids are bound by auth/*.js. If a module fails to load or an import
    // breaks, the elements still exist in the static HTML but nothing answers a
    // click — which is the failure mode FE-012's module split could reintroduce.
    await expect(page.locator("#btn-generate")).toBeEnabled();
    await expect(page.locator("#key-id")).toHaveValue("laptop");

    await page.locator("#btn-generate").click();

    // Without the wasm bundle the handler must degrade into a message in
    // #keygen-output rather than throwing.
    // Clicking it reveals #keygen-output and, with the wasm bundle absent here,
    // the catch in keygen.js writes "❌ ..." — proving the handler is wired and
    // degrades instead of throwing.
    await expect(page.locator("#keygen-output")).toContainText("❌", { timeout: 15_000 });
    expect(errors).toEqual([]);
  });
});