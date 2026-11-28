# Build context is the REPOSITORY ROOT (see backend/docker-compose.yml).
# Two stages need two different trees: the CSS stage reads frontend/, the Rust
# stage reads backend/app/. A context of backend/ cannot reach frontend/.

# FE-003: CSS flattening stage — Node 20 + PostCSS + postcss-preset-env
# Flattens CSS Nesting Level 1 (e.g. `#name-section { #name-span {} }`,
# `.glass { &::before }`) from frontend/static/*.css → frontend/static/dist/*.css.
# Source files remain nested (author input); only the flattened dist is served.
# FE-006a: the same stage builds the Tailwind v4 utilities (tailwind.input.css
# → frontend/static/dist/tailwind.css). Preflight is excluded there, so the
# hand-rolled CSS is not reset.
FROM node:20-alpine AS css-builder
# The stage mirrors the frontend/ layout, so package.json's relative script
# paths (`static/*.css`, `tailwind.input.css`) work unchanged.
WORKDIR /build

# Reproducible install from the committed lockfile — never `npm install`.
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --no-audit --no-fund

COPY frontend/postcss.config.cjs ./
# FE-006a: the Tailwind input sits at the frontend root rather than in static/,
# so the `static/*.css` glob stays author-CSS only. Its `@source` scans
# static/*.html, so the markup must be present for utility detection.
COPY frontend/tailwind.input.css ./
COPY frontend/static/*.css frontend/static/*.html ./static/
RUN npm run build:css && \
    for f in ./static/dist/*.css; do \
        test -s "$f" || { echo "FE-003: empty flattened CSS: $f"; exit 1; }; \
    done && \
    for f in ./static/dist/*.css; do \
        case "$f" in *tailwind.css) continue;; esac; \
        if grep -n '&' "$f"; then \
            echo "FE-003: nesting operators remain after flattening: $f"; exit 1; \
        fi; \
    done && \
    ls -lh ./static/dist/

# Build stage
FROM rust:alpine AS builder

# Install build deps: UPX (compressor), postgresql-dev (static libpq for diesel)
RUN apk add --no-cache upx postgresql-dev

# Detect native architecture
RUN ARCH=$(uname -m) && \
    case "$ARCH" in \
        x86_64)  TARGET="x86_64-unknown-linux-musl" ;; \
        aarch64) TARGET="aarch64-unknown-linux-musl" ;; \
        armv7l)  TARGET="armv7-unknown-linux-musleabihf" ;; \
        *)       echo "unsupported arch: $ARCH" && exit 1 ;; \
    esac && \
    rustup target add "$TARGET" && \
    echo "$TARGET" > /target.txt

WORKDIR /app

# ── Dependency cache layer ──
COPY backend/app/Cargo.toml backend/app/Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN TARGET=$(cat /target.txt) && cargo build --release --target "$TARGET"
RUN rm -rf src

# ── Source build layer ──
# Remove stale dummy artifacts (binary + fingerprint) to force
# cargo to re-link the real binary. All compiled dependencies
# are preserved in the cache.
COPY backend/app/src/ src/
# Static assets land at /frontend/static in the image, which is exactly the
# CWD-relative prefix the compiled-in STATIC_ROUTES table uses.
COPY frontend/static/*.html frontend/static/*.js /frontend/static/
# FE-005/FE-012: nested ES modules for the index and auth pages (served via
# STATIC_ROUTES). These are not passed through uglify-js below; it cannot
# parse import/export.
COPY frontend/static/pages/ /frontend/static/pages/
COPY frontend/static/auth/ /frontend/static/auth/
COPY frontend/static/assets/ /frontend/static/assets/
# FALCON WASM for the auth tool (tracked build artifacts). Served by
# STATIC_ROUTES at /falcon_wasm.js and /falcon_wasm_bg.wasm.
COPY backend/app/crates/falcon-wasm/pkg/ /backend/app/crates/falcon-wasm/pkg/
# Book is embedded in the binary (include_bytes!) — this copy is optional,
# kept only so OPENING_BOOK=/frontend/static/filtered.bin can override at runtime.
COPY backend/app/src/games/chess/filtered.bin /frontend/static/filtered.bin

# FE-003: Serve only the flattened CSS. Source CSS stays out of the runtime
# image, so unflattened nesting cannot be served in production.
COPY --from=css-builder /build/static/dist /frontend/static/dist

# ── Minify static assets (builder stage only — zero impact on final image) ──
RUN apk add --no-cache nodejs npm && \
    npm install -g html-minifier-terser uglify-js csso-cli svgo && \
    for f in /frontend/static/*.html; do \
        html-minifier-terser --collapse-whitespace --remove-comments \
          --remove-optional-tags -o "$f" "$f" 2>/dev/null || true; \
    done && \
    for f in /frontend/static/dist/*.css; do \
        csso "$f" --output "$f" 2>/dev/null || true; \
    done && \
    for f in /frontend/static/*.js; do \
        uglifyjs "$f" --compress --mangle -o "$f" 2>/dev/null || true; \
    done && \
    for f in /frontend/static/assets/*.svg; do \
        svgo --quiet --precision 2 -i "$f" -o "$f" 2>/dev/null || true; \
    done && \
    apk del nodejs npm && \
    rm -rf /root/.npm /root/.cache /usr/lib/node_modules

RUN TARGET=$(cat /target.txt) && \
    rm -f "target/$TARGET/release/backend" && \
    rm -rf "target/$TARGET/release/.fingerprint/backend-"* && \
    cargo build --release --target "$TARGET" && \
    cp "target/$TARGET/release/backend" /server && \
    upx --best --lzma /server

# ── Final run stage (completely empty) ──
FROM scratch AS runner

# Copy the fully static binary. Named /server, not /backend: /backend/app/... holds
# the CWD-relative paths STATIC_ROUTES resolves at runtime.
COPY --from=builder /server /server
COPY --from=builder /frontend /frontend
# FALCON WASM (auth tool) — keep the pkg path in sync with STATIC_ROUTES.
COPY --from=builder /backend/app/crates/falcon-wasm/pkg /backend/app/crates/falcon-wasm/pkg

# Add CA certificates here if your app makes outbound HTTPS/TLS calls:
# RUN apk add --no-cache ca-certificates && cp /etc/ssl/certs/ca-certificates.crt /

EXPOSE 8000
CMD ["/server"]
