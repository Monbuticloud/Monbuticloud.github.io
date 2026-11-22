// Minimal static server for the Playwright smoke tests.
//
// Serves backend/static/ at the root, which mirrors how the backend exposes
// page assets: the HTML links bare relative URLs (./common.css, ./auth/main.js,
// ./pages/index/index.js), so the directory has to be the web root.
//
// Deliberately dependency-free — the only thing the smoke tests need is
// "these files, served over HTTP, with correct content types".

import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { basename, extname, join, normalize, sep } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../static/", import.meta.url));
const PORT = Number(process.env.PORT ?? 4173);

const CONTENT_TYPES = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".avif": "image/avif",
  ".wasm": "application/wasm",
  ".txt": "text/plain; charset=utf-8",
  ".xml": "application/xml; charset=utf-8",
};

const server = createServer(async (req, res) => {
  try {
    const url = new URL(req.url ?? "/", "http://localhost");
    let pathname = decodeURIComponent(url.pathname);
    if (pathname.endsWith("/")) pathname += "index.html";

    // Resolve first, then confirm the result is still inside ROOT — this is what
    // stops ../ traversal rather than trying to pattern-match the raw URL.
    const target = normalize(join(ROOT, pathname));
    if (!target.startsWith(ROOT.endsWith(sep) ? ROOT : ROOT + sep)) {
      res.writeHead(403, { "content-type": "text/plain" });
      res.end("forbidden");
      return;
    }

    // Mirror src/api/html.rs::resolve_content: for CSS under static/, serve
    // static/dist/<name> when it exists (that is what the image ships) and fall
    // back to source. Without this, a page linking tailwind.css 404s here even
    // though the Rust server would serve it.
    let file = target;
    if (pathname.endsWith(".css")) {
      const fromDist = join(ROOT, "dist", basename(target));
      const distOk = await stat(fromDist)
        .then((s) => s.isFile())
        .catch(() => false);
      if (distOk) file = fromDist;
    }

    const info = await stat(file);
    if (info.isDirectory()) {
      res.writeHead(301, { location: `${pathname}/` });
      res.end();
      return;
    }

    const body = await readFile(file);
    res.writeHead(200, {
      "content-type": CONTENT_TYPES[extname(target)] ?? "application/octet-stream",
      "content-length": body.length,
    });
    res.end(body);
  } catch {
    res.writeHead(404, { "content-type": "text/plain" });
    res.end("not found");
  }
});

server.listen(PORT, () => {
  console.log(`static server listening on http://localhost:${PORT} (root: ${ROOT})`);
});