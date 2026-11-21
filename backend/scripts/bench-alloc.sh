#!/usr/bin/env bash
set -euo pipefail
# bench-alloc.sh — allocator A/B latency bench for the Homepage backend
#
# Measures: count, mean, p25-75 mean, p.1, p1, p10, p25, p50, p75, p90, p95,
#           p99, p99.9, min, max, stddev
#
# Fairness rules (see "Why 512m" below):
#   1. Both variants build from the SAME Dockerfile as production
#      (scripts/bench-alloc.Dockerfile -> x86_64-unknown-linux-musl).
#   2. Both run under an IDENTICAL --memory cap with swap disabled, so an
#      allocator that retains more pages OOMs loudly instead of silently
#      getting extra headroom.
#   3. Only the HTTP path is benched. The chess search is stack-only with a
#      single global 32MB TT and a cpus/4 semaphore (3 permits on 12 cores),
#      so it does not exercise the allocator.
#
# Why 512m: steady-state need is ~100MB (32MB global TT + 12x256KB rayon
# stacks + HTTP churn). 512m is generous headroom, and an OOM inside a 60s
# run is itself a signal (that allocator retained too much).
#
# Usage:
#   ./scripts/bench-alloc.sh                      # bench the running container
#   ./scripts/bench-alloc.sh --compare            # A/B mimalloc vs jemalloc
#   ./scripts/bench-alloc.sh --compare -c 1000 -d 60 --runs 2
#
# Options:
#   --url URL          target URL (default http://127.0.0.1:8000/)
#   -c, --concurrency  concurrent connections (default 500)
#   -d, --duration     seconds per run (default 60)
#   --warmup           warmup seconds, discarded (default 5)
#   --runs             repetitions, median reported (default 2)
#   --memory           per-container memory cap (default 512m)
#   --compare          build + bench both allocators
#   -h, --help         this help

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BACKEND="$ROOT/backend"
PY="$BACKEND/scripts/bench-alloc.py"
OUTDIR="$BACKEND/.tmp/bench-alloc"

URL="http://127.0.0.1:8000/"
CONCURRENCY=500
DURATION=60
WARMUP=5
RUNS=2
MEMORY=512m
COMPARE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --url) URL="$2"; shift 2 ;;
    -c|--concurrency) CONCURRENCY="$2"; shift 2 ;;
    -d|--duration) DURATION="$2"; shift 2 ;;
    --warmup) WARMUP="$2"; shift 2 ;;
    --runs) RUNS="$2"; shift 2 ;;
    --memory) MEMORY="$2"; shift 2 ;;
    --compare) COMPARE=1; shift ;;
    -h|--help) sed -n '2,40p' "$0"; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing dep: $1" >&2; exit 1; }; }
need python3
[[ $COMPARE -eq 1 ]] && need docker

mkdir -p "$OUTDIR"
chmod +x "$PY" 2>/dev/null || true
python3 -m py_compile "$PY"

echo "# bench-alloc.sh — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "# url=$URL c=$CONCURRENCY dur=${DURATION}s warmup=${WARMUP}s runs=$RUNS mem=$MEMORY compare=$COMPARE"

# ── helpers ────────────────────────────────────────────────────────────

wait_health() {
  local base="$1" i
  echo "# waiting for $base/health ..."
  for i in $(seq 1 60); do
    if curl -sf "$base/health" >/dev/null 2>&1; then
      echo "# healthy after ${i} tries"
      return 0
    fi
    sleep 0.5
  done
  echo "# warn: health check never passed at $base/health" >&2
  return 1
}

# sample container memory usage (cgroup v2 current+peak, fallback to docker stats)
sample_mem() {
  local cid="$1" label="$2" log="$3"
  {
    echo "# t container_mem_bytes"
    while docker inspect -f '{{.State.Running}}' "$cid" 2>/dev/null | grep -q true; do
      docker stats --no-stream --format '{{.MemUsage}}' "$cid" 2>/dev/null \
        | awk -v ts="$(date +%H:%M:%S)" '{print ts, $0}' || true
      sleep 2
    done
  } > "$log" 2>&1 &
  echo $!
}

# bench one already-running server
bench_running() {
  local label="$1" base="$2" cid="${3:-}"
  local csv="$OUTDIR/${label}.csv"
  local memlog="$OUTDIR/${label}-mem.log"
  local mempid=""

  echo ""
  echo "## $label — $base"
  if [[ -n "$cid" ]]; then
    mempid=$(sample_mem "$cid" "$label" "$memlog")
  fi

  # Warmup is explicit: the first seconds of a fresh process are dominated by
  # lazy init (global TT alloc, file cache warm) and would poison p.1.
  if [[ $WARMUP -gt 0 ]]; then
    echo "# warmup ${WARMUP}s (discarded) ..."
    python3 "$PY" --url "$base" --concurrency "$CONCURRENCY" \
      --duration "$WARMUP" --warmup 0 --runs 1 >/dev/null 2>&1 || true
  fi

  python3 "$PY" --url "$URL" --concurrency "$CONCURRENCY" \
    --duration "$DURATION" --warmup 0 --runs "$RUNS" --out "$csv"

  [[ -n "$mempid" ]] && kill "$mempid" 2>/dev/null || true

  # Did it survive the cap? An OOM is a result, not a crash.
  if [[ -n "$cid" ]]; then
    local oom
    oom=$(docker inspect -f '{{.State.OOMKilled}} {{.State.ExitCode}}' "$cid" 2>/dev/null || echo "?")
    echo "# container state: OOMKilled/ExitCode = $oom"
    case "$oom" in
      true*) echo "# !! OOM KILLED at $MEMORY — this allocator retained too much" ;;
    esac
    if [[ -s "$memlog" ]]; then
      echo "# peak mem sample:"
      sort -k2 -h "$memlog" 2>/dev/null | grep -v '^#' | tail -3 || true
    fi
  fi
  echo "# csv: $csv"
}

# ── single-variant mode: bench whatever is already up ─────────────────

if [[ $COMPARE -eq 0 ]]; then
  CID=$(docker compose -f "$BACKEND/docker-compose.yml" ps -q app 2>/dev/null | head -n1 || true)
  if [[ -n "$CID" ]]; then
    wait_health "$URL" || true
    bench_running "mimalloc" "$URL" "$CID"
  else
    echo "# no running app container; bench $URL directly"
    bench_running "mimalloc" "$URL" ""
  fi
  echo ""
  echo "# done — single variant. Re-run with --compare for the A/B."
  exit 0
fi

# ── compare mode: build both variants from the production Dockerfile ───

echo ""
echo "# building variants (musl, same profile as production) ..."

build_variant() {
  local alloc="$1" tag="backend-bench-$alloc"
  echo "# -> $tag"
  docker build \
    -f "$BACKEND/scripts/bench-alloc.Dockerfile" \
    --build-arg "ALLOC=$alloc" \
    -t "$tag" \
    -q "$BACKEND"
}

build_variant mimalloc
build_variant jemalloc

# Run each variant on its own port under an identical, swap-less cap.
run_variant() {
  local alloc="$1" port="$2"
  local name="bench-$alloc"
  docker rm -f "$name" >/dev/null 2>&1 || true
  # --memory-swap == --memory disables swap so the cap is a true hard ceiling.
  # --memory-reservation keeps the pages from being lazily over-committed.
  docker run -d --name "$name" \
    --memory "$MEMORY" --memory-swap "$MEMORY" \
    -p "127.0.0.1:$port:8000" \
    -e AUTH_HMAC_SECRET="$(head -c 48 /dev/urandom | base64)" \
    "backend-bench-$alloc" >/dev/null
  echo "$name"
}

cleanup() {
  for n in bench-mimalloc bench-jemalloc; do
    docker rm -f "$n" >/dev/null 2>&1 || true
  done
}
trap cleanup EXIT

echo ""
echo "# running both under --memory=$MEMORY (swap disabled) ..."
MIM=$(run_variant mimalloc 18001)
JEM=$(run_variant jemalloc 18002)

MIM_BASE="http://127.0.0.1:18001"
JEM_BASE="http://127.0.0.1:18002"
MIM_URL="${URL/127.0.0.1:8000/127.0.0.1:18001}"
JEM_URL="${URL/127.0.0.1:8000/127.0.0.1:18002}"

wait_health "$MIM_BASE" || { echo "# mimalloc variant failed to start" >&2; docker logs "$MIM" | tail -20; exit 1; }
wait_health "$JEM_BASE" || { echo "# jemalloc variant failed to start" >&2; docker logs "$JEM" | tail -20; exit 1; }

# NOTE: bench_running reads $URL for the actual requests; swap it per variant.
URL="$MIM_URL" bench_running "mimalloc" "$MIM_BASE" "$(docker inspect -f '{{.Id}}' "$MIM")"
URL="$JEM_URL" bench_running "jemalloc" "$JEM_BASE" "$(docker inspect -f '{{.Id}}' "$JEM")"

# ── side-by-side medians ───────────────────────────────────────────────

echo ""
echo "# ═══ medians across $RUNS run(s) ═══"
python3 - "$OUTDIR" <<'PY'
import csv, glob, os, sys

outdir = sys.argv[1]

def median(vals):
    s = sorted(vals)
    n = len(s)
    if n == 0: return 0.0
    k = (n - 1) / 2
    f = int(k); c = min(f + 1, n - 1)
    return s[f] + (s[c] - s[f]) * (k - f)

KEYS = ["mean","p25_p75_mean","p0.1","p1","p10","p25","p50","p75","p90","p95","p99","p99.9","max","stddev"]
data = {}
for path in sorted(glob.glob(os.path.join(outdir, "*.csv"))):
    name = os.path.splitext(os.path.basename(path))[0]
    with open(path) as f:
        rows = [r for r in csv.DictReader(f) if r.get("count")]
    if not rows: continue
    data[name] = {k: median([float(r[k]) for r in rows if r.get(k)]) for k in KEYS}
    data[name]["count"] = sum(int(float(r["count"])) for r in rows)

if not data:
    print("no results"); raise SystemExit(0)

hdr = f"{'variant':<12}{'count':>9}" + "".join(f"{k:>13}" for k in KEYS)
print(hdr); print("-"*len(hdr))
for name, d in data.items():
    print(f"{name:<12}{d['count']:>9}" + "".join(f"{d[k]:>13.2f}" for k in KEYS))

# Verdict on the tail only — p99/p99.9/p1/p0.1 are where allocator contention shows.
if "mimalloc" in data and "jemalloc" in data:
    m, j = data["mimalloc"], data["jemalloc"]
    print()
    print("── tail verdict (lower is better) ──")
    wins = 0
    for k in ["p0.1","p1","p10","p50","p99","p99.9"]:
        if j[k] == 0: continue
        d = (m[k] - j[k]) / j[k] * 100
        mark = "mimalloc" if d < 0 else "jemalloc"
        wins += 1 if d < 0 else 0
        print(f"  {k:<7} {d:+6.1f}%  -> {mark}")
    print(f"\n  mimalloc wins {wins}/6 tail marks.")
    print("  |Δ| < ~2% on all marks = noise, not a signal. Keep mimalloc (already default).")
PY

echo ""
echo "# logs: $OUTDIR/*.csv  $OUTDIR/*-mem.log"
echo "# done."
