# Build stage
FROM rust:alpine AS builder

# Install UPX (executable compressor). Alpine already ships musl natively.
RUN apk add --no-cache upx

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
COPY static/*.html static/*.css static/*.js static/assets/icons.svg static/assets/images/laptop-fullstack-optim.avif static/assets/images/laptop-fullstack-optim_hd.avif /static/
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
