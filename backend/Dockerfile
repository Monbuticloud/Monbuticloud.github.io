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
COPY static/*.html static/*.css static/*.js /static/
COPY static/assets/ /static/assets/

# ── Minify static assets (builder stage only — zero impact on final image) ──
RUN apk add --no-cache nodejs npm && \
    npm install -g html-minifier-terser uglify-js csso-cli svgo && \
    for f in /static/*.html; do \
        html-minifier-terser --collapse-whitespace --remove-comments \
          --remove-optional-tags -o "$f" "$f" 2>/dev/null || true; \
    done && \
    for f in /static/*.css; do \
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

# Add CA certificates here if your app makes outbound HTTPS/TLS calls:
# RUN apk add --no-cache ca-certificates && cp /etc/ssl/certs/ca-certificates.crt /

EXPOSE 8000
CMD ["/backend"]
