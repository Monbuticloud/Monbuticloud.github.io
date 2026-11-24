# bench-alloc.Dockerfile — allocator A/B variant builder
# ARG selects the allocator at compile time; everything else mirrors Dockerfile
# (musl target, same profile) so the A/B measures the artifact you actually ship.
#
#   docker build -f backend/app/scripts/bench-alloc.Dockerfile \
#     --build-arg ALLOC=mimalloc -t backend-bench-mimalloc .   # from the repo root
#   docker build -f backend/app/scripts/bench-alloc.Dockerfile \
#     --build-arg ALLOC=jemalloc -t backend-bench-jemalloc .   # from the repo root
FROM rust:alpine AS builder

RUN apk add --no-cache postgresql-dev

RUN ARCH=$(uname -m) && \
    case "$ARCH" in \
        x86_64)  TARGET="x86_64-unknown-linux-musl" ;; \
        aarch64) TARGET="aarch64-unknown-linux-musl" ;; \
        *)       echo "unsupported arch: $ARCH" && exit 1 ;; \
    esac && \
    rustup target add "$TARGET" && \
    echo "$TARGET" > /target.txt

WORKDIR /app

# ── Allocator selection ────────────────────────────────────────────────
# jemalloc is optional and off by default; ALLOC=jemalloc turns it on.
# tikv-jemallocator needs a C toolchain, which rust:alpine already has.
COPY backend/app/Cargo.toml backend/app/Cargo.lock* ./
RUN if ! grep -q 'tikv-jemallocator' Cargo.toml; then \
      printf '\n[features]\njemalloc = ["tikv-jemallocator"]\n\n[dependencies.tikv-jemallocator]\nversion = "0.6"\noptional = true\n' >> Cargo.toml; \
    fi

COPY backend/app/src/ src/
COPY frontend/static/ /frontend/static/
COPY backend/app/src/games/chess/filtered.bin /frontend/static/filtered.bin

# Gate the global_allocator on the feature, so one source tree builds both.
RUN python3 - <<'PY' || apk add --no-cache python3
import pathlib
p = pathlib.Path("src/main.rs")
t = p.read_text()
old = "#[global_allocator]\nstatic GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;"
new = (
    '#[cfg(not(feature = "jemalloc"))]\n'
    "#[global_allocator]\n"
    "static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;\n"
    '#[cfg(feature = "jemalloc")]\n'
    "#[global_allocator]\n"
    "static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;"
)
if old in t and "feature = \"jemalloc\"" not in t:
    p.write_text(t.replace(old, new))
    print("allocator gated")
PY

ARG ALLOC=mimalloc
RUN TARGET=$(cat /target.txt) && \
    if [ "$ALLOC" = "jemalloc" ]; then FEAT="--features jemalloc"; else FEAT=""; fi && \
    cargo build --release --target "$TARGET" $FEAT && \
    cp "target/$TARGET/release/backend" /backend

# ── Run stage: scratch, same as production ─────────────────────────────
FROM scratch AS runner
COPY --from=builder /backend /backend
COPY --from=builder /frontend /frontend
EXPOSE 8000
CMD ["/backend"]
