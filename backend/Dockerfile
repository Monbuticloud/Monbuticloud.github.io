# FE-003: CSS flattening stage — Node 20 + PostCSS + postcss-preset-env
# Flattens CSS Nesting Level 1 (e.g. `#name-section { #name-span {} }`,
# `.glass { &::before }`) from static/*.css → static/dist/*.css.
# Source files remain nested (author input); only the flattened dist is served.
FROM node:20-alpine AS css-builder
WORKDIR /build

# Reproducible install from the committed lockfile — never `npm install`.
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund

COPY postcss.config.cjs ./
# Only CSS is needed for flattening.
COPY static/*.css ./static/
RUN npm run build:css && \
    for f in ./static/dist/*.css; do \
        test -s "$f" || { echo "FE-003: empty flattened CSS: $f"; exit 1; }; \
    done && \
    if grep -R -n '&' ./static/dist/*.css; then \
        echo "FE-003: nesting operators remain after flattening"; exit 1; \
    fi && \
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
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN TARGET=$(cat /target.txt) && cargo build --release --target "$TARGET"
RUN rm -rf src

# ── Source build layer ──
# Remove stale dummy artifacts (binary + fingerprint) to force
# cargo to re-link the real binary. All compiled dependencies
# are preserved in the cache.
COPY src/ src/
COPY static/*.html static/*.js /static/
# FE-005: nested ES modules for the index page (served via STATIC_ROUTES).
# These are not passed through uglify-js below; it cannot parse import/export.
COPY static/pages/ /static/pages/
COPY static/assets/ /static/assets/
# FALCON WASM for the auth tool (tracked build artifacts). Served by
# STATIC_ROUTES at /falcon_wasm.js and /falcon_wasm_bg.wasm.
COPY crates/falcon-wasm/pkg/ /crates/falcon-wasm/pkg/
# Book is embedded in the binary (include_bytes!) — this copy is optional,
# kept only so OPENING_BOOK=/static/filtered.bin can override at runtime.
COPY src/games/chess/filtered.bin /static/filtered.bin

# FE-003: Serve only the flattened CSS. Source CSS stays out of the runtime
# image, so unflattened nesting cannot be served in production.
COPY --from=css-builder /build/static/dist /static/dist

# ── Minify static assets (builder stage only — zero impact on final image) ──
RUN apk add --no-cache nodejs npm && \
    npm install -g html-minifier-terser uglify-js csso-cli svgo && \
    for f in /static/*.html; do \
        html-minifier-terser --collapse-whitespace --remove-comments \
          --remove-optional-tags -o "$f" "$f" 2>/dev/null || true; \
    done && \
    for f in /static/dist/*.css; do \
        csso "$f" --output "$f" 2>/dev/null || true; \
    done && \
    for f in /static/*.js; do \
        uglifyjs "$f" --compress --mangle -o "$f" 2>/dev/null || true; \
    done && \
    for f in /static/assets/*.svg; do \
        svgo --quiet --precision 2 -i "$f" -o "$f" 2>/dev/null || true; \
    done && \
    apk del nodejs npm && \
    rm -rf /root/.npm /root/.cache /usr/lib/node_modules

RUN TARGET=$(cat /target.txt) && \
    rm -f "target/$TARGET/release/backend" && \
    rm -rf "target/$TARGET/release/.fingerprint/backend-"* && \
    cargo build --release --target "$TARGET" && \
    cp "target/$TARGET/release/backend" /backend && \
    upx --best --lzma /backend

# ── Final run stage (completely empty) ──
FROM scratch AS runner

# Copy the fully static binary
COPY --from=builder /backend /backend
COPY --from=builder /static static/
# FALCON WASM (auth tool) — keep the pkg path in sync with STATIC_ROUTES.
COPY --from=builder /crates/falcon-wasm/pkg /crates/falcon-wasm/pkg

# Add CA certificates here if your app makes outbound HTTPS/TLS calls:
# RUN apk add --no-cache ca-certificates && cp /etc/ssl/certs/ca-certificates.crt /

EXPOSE 8000
CMD ["/backend"]
