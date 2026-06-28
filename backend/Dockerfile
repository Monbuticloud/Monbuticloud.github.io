# Build stage
FROM rust:bullseye AS builder

# Install musl tools (musl-gcc linker) and UPX (executable compressor)
RUN apt-get update && apt-get install -y musl-tools upx-ucl && rm -rf /var/lib/apt/lists/*

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
RUN TARGET=$(cat /target.txt) && \
    rm -f "target/$TARGET/release/backend" && \
    rm -rf "target/$TARGET/release/.fingerprint/backend-"* && \
    cargo build --release --target "$TARGET" && \
    cp "target/$TARGET/release/backend" /backend && \
    upx-ucl --best /backend

# ── Final run stage (completely empty) ──
FROM scratch AS runner

# Copy the fully static binary
COPY --from=builder /backend /backend

# Copy CA certificates if your app makes outbound HTTPS/TLS calls
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

EXPOSE 8000
CMD ["/backend"]
