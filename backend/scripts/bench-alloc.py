#!/usr/bin/env python3
"""
bench-alloc.py — latency bench for Homepage backend (allocator comparison)
Measures: mean, p25-p75 mean, p25, p10, p1, p.1 (+ full table for context)
Pure-function percentile core, ThreadPool live sampling, no external deps.
"""
import argparse
import http.client
import math
import statistics
import threading
import time
import urllib.parse
from concurrent.futures import ThreadPoolExecutor, as_completed


# ── Pure helpers (testable, no side effects) ───────────────────────────

def percentile(sorted_ms: list[float], p: float) -> float:
    """Linear-interpolated percentile. p in [0,100], e.g. 0.1 for p.1"""
    n = len(sorted_ms)
    if n == 0:
        return 0.0
    if n == 1:
        return float(sorted_ms[0])
    k = (n - 1) * p / 100.0
    f = math.floor(k)
    c = math.ceil(k)
    if f == c:
        return float(sorted_ms[int(k)])
    d0 = sorted_ms[int(f)] * (c - k)
    d1 = sorted_ms[int(c)] * (k - f)
    return d0 + d1


def interquartile_mean(sorted_ms: list[float], p25: float, p75: float) -> float:
    """Mean of values within [p25, p75] inclusive. Pure."""
    if not sorted_ms:
        return 0.0
    lo, hi = p25, p75
    # inclusive range — handles duplicates correctly
    window = [x for x in sorted_ms if lo <= x <= hi]
    if not window:
        # fallback: slice by rank when no value falls exactly in range (tiny n)
        n = len(sorted_ms)
        i_lo = max(0, math.ceil(n * 25 / 100) - 1)
        i_hi = min(n - 1, math.floor(n * 75 / 100) - 1)
        window = sorted_ms[i_lo : i_hi + 1]
    return sum(window) / len(window) if window else 0.0


def summarize(latencies_ms: list[float]) -> dict:
    """Pure: latencies -> stats dict with all requested percentiles."""
    if not latencies_ms:
        return {}
    s = sorted(latencies_ms)
    n = len(s)
    mean_v = sum(s) / n
    std_v = statistics.pstdev(s) if n > 1 else 0.0
    p01 = percentile(s, 0.1)
    p1 = percentile(s, 1)
    p10 = percentile(s, 10)
    p25 = percentile(s, 25)
    p50 = percentile(s, 50)
    p75 = percentile(s, 75)
    p90 = percentile(s, 90)
    p95 = percentile(s, 95)
    p99 = percentile(s, 99)
    p999 = percentile(s, 99.9)
    iqm = interquartile_mean(s, p25, p75)
    return {
        "count": n,
        "min": s[0],
        "max": s[-1],
        "mean": mean_v,
        "stddev": std_v,
        "p0.1": p01,
        "p1": p1,
        "p10": p10,
        "p25": p25,
        "p50": p50,
        "p75": p75,
        "p90": p90,
        "p95": p95,
        "p99": p99,
        "p99.9": p999,
        "p25_p75_mean": iqm,
    }


# ── Sampling (impure, isolated) ────────────────────────────────────────

def _parse_url(url: str):
    u = urllib.parse.urlparse(url)
    host = u.hostname or "127.0.0.1"
    port = u.port or (443 if u.scheme == "https" else 80)
    path = u.path or "/"
    if u.query:
        path += "?" + u.query
    use_ssl = u.scheme == "https"
    return host, port, path, use_ssl


def collect(url: str, concurrency: int, duration_s: float) -> list[float]:
    """Collect per-request latencies (ms) for duration_s at concurrency. Blocking."""
    host, port, path, use_ssl = _parse_url(url)
    latencies: list[float] = []
    lock = threading.Lock()
    errors = 0
    stop_at = time.monotonic() + duration_s
    conn_cls = http.client.HTTPSConnection if use_ssl else http.client.HTTPConnection

    def worker():
        nonlocal errors
        # one persistent connection per worker (keep-alive)
        conn = None
        while time.monotonic() < stop_at:
            if conn is None:
                try:
                    conn = conn_cls(host, port, timeout=5)
                except Exception:
                    with lock:
                        errors += 1
                    time.sleep(0.01)
                    continue
            start = time.perf_counter()
            try:
                conn.request("GET", path, headers={"Connection": "keep-alive"})
                resp = conn.getresponse()
                resp.read()  # drain
                # treat 5xx as error but still record latency
                elapsed = (time.perf_counter() - start) * 1000.0
                with lock:
                    latencies.append(elapsed)
                    if resp.status >= 500:
                        errors += 1
                # keep connection for next iter
            except Exception:
                with lock:
                    errors += 1
                try:
                    conn.close()
                except Exception:
                    pass
                conn = None
                # small backoff to avoid tight loop on refused
                time.sleep(0.005)
        if conn is not None:
            try:
                conn.close()
            except Exception:
                pass

    with ThreadPoolExecutor(max_workers=concurrency) as pool:
        futs = [pool.submit(worker) for _ in range(concurrency)]
        for f in as_completed(futs):
            f.result()
    # errors not fatal — caller can inspect len vs expected
    return latencies


def bench_once(url: str, concurrency: int, duration_s: float, warmup_s: float) -> dict:
    """Warmup (discard) + bench. Returns summarize() dict."""
    if warmup_s > 0:
        _ = collect(url, concurrency, warmup_s)
    lats = collect(url, concurrency, duration_s)
    return summarize(lats)


def median_of_runs(runs: list[dict], key: str) -> float:
    vals = sorted(r[key] for r in runs if r)
    if not vals:
        return 0.0
    return percentile(vals, 50)


def format_row(d: dict) -> str:
    return (
        f"{d['count']:>7}  "
        f"{d['mean']:7.2f}  {d['p25_p75_mean']:7.2f}  "
        f"{d['p0.1']:7.2f}  {d['p1']:7.2f}  {d['p10']:7.2f}  {d['p25']:7.2f}  "
        f"{d['p50']:7.2f}  {d['p75']:7.2f}  {d['p90']:7.2f}  {d['p95']:7.2f}  {d['p99']:7.2f}  {d['p99.9']:7.2f}  "
        f"{d['min']:7.2f}  {d['max']:7.2f}  {d['stddev']:7.2f}"
    )


HEADER = (
    "  count     mean  p25-75m     p.1       p1      p10      p25      p50      p75      p90      p95      p99    p99.9      min      max   stddev"
)


def main():
    ap = argparse.ArgumentParser(description="Homepage alloc bench — tail percentiles")
    ap.add_argument("--url", default="http://127.0.0.1:8000/", help="URL to bench")
    ap.add_argument("--concurrency", type=int, default=50)
    ap.add_argument("--duration", type=float, default=30, help="bench seconds per run")
    ap.add_argument("--warmup", type=float, default=5, help="warmup seconds (discarded)")
    ap.add_argument("--runs", type=int, default=3, help="repetitions, median reported")
    ap.add_argument("--out", default="", help="optional CSV out path")
    args = ap.parse_args()

    print(f"# bench-alloc.py — {args.url} c={args.concurrency} dur={args.duration}s warmup={args.warmup}s runs={args.runs}")
    print(f"# {time.strftime('%Y-%m-%d %H:%M:%S')} — python {urllib.parse.__name__}")
    print(HEADER)
    print("-" * len(HEADER))

    runs = []
    for i in range(1, args.runs + 1):
        print(f"# run {i}/{args.runs} ...", flush=True)
        s = bench_once(args.url, args.concurrency, args.duration, args.warmup)
        if not s:
            print(f"run {i}: no samples (is server up at {args.url}?)")
            continue
        runs.append(s)
        print(f"run {i}:  {format_row(s)}")

    if not runs:
        print("no successful runs")
        return

    if len(runs) > 1:
        # median across runs per metric
        keys = ["mean", "p25_p75_mean", "p0.1", "p1", "p10", "p25", "p50", "p75", "p90", "p95", "p99", "p99.9", "min", "max", "stddev"]
        med = {k: median_of_runs(runs, k) for k in keys}
        med["count"] = int(sum(r["count"] for r in runs) / len(runs))
        print("-" * len(HEADER))
        print(f"median: {format_row(med)}  (median of {len(runs)} runs)")
        print(f"\n# median p25-p75 mean={med['p25_p75_mean']:.2f}ms  p.1={med['p0.1']:.2f}ms  p1={med['p1']:.2f}ms  p10={med['p10']:.2f}ms  p25={med['p25']:.2f}ms")

    if args.out:
        import csv, pathlib

        p = pathlib.Path(args.out)
        p.parent.mkdir(parents=True, exist_ok=True)
        with p.open("w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=list(runs[0].keys()))
            w.writeheader()
            w.writerows(runs)
        print(f"# csv -> {p}")


if __name__ == "__main__":
    main()
