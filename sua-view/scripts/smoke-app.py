#!/usr/bin/env python3
"""Run the actual native app and retain timing/functional evidence locally."""
import argparse
import json
import os
import statistics
import subprocess
import time
from pathlib import Path


def run(app, book, output, run_number, stress=False):
    output.mkdir(parents=True, exist_ok=True)
    metrics = (output / f"smoke-{run_number}.jsonl").resolve()
    if metrics.exists():
        raise SystemExit(f"Choose a new output directory: {metrics} already exists")
    env = dict(os.environ, SUA_VIEW_TESTING="1", SUA_VIEW_METRICS=str(metrics))
    started = time.monotonic()
    mode = "--stress-test" if stress else "--smoke-test"
    process = subprocess.Popen([str(app), mode, str(book), "--smoke-exit"], env=env,
                               stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    rss = []
    try:
        while process.poll() is None:
            if time.monotonic() - started > 30:
                raise TimeoutError("Native smoke test exceeded 30 seconds")
            sample = subprocess.run(["ps", "-o", "rss=", "-p", str(process.pid)],
                                    capture_output=True, text=True)
            if sample.stdout.strip():
                rss.append(int(sample.stdout.strip()))
            time.sleep(.05)
        stderr = process.communicate()[1].decode(errors="replace")
        if process.returncode:
            raise RuntimeError(f"App exit {process.returncode}: {stderr}")
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
    events = [json.loads(line) for line in metrics.read_text().splitlines()]
    errors = [e for e in events if e["event"] in ("open_error", "page_error")]
    assert not errors, errors
    assert any(e["event"] == "ready" for e in events), events
    opened = next(e for e in events if e["event"] == "opened")
    assert opened["count"] >= 3, "Smoke fixture needs at least three pages"
    assert opened["index"] == 0, opened
    pages = [e for e in events if e["event"] in ("first_page", "page")]
    assert [e["index"] for e in pages[:3]] == ([0, 1, 2] if stress else [0, 1, 0]), pages
    assert any(e["displayed"] == 2 for e in pages), pages
    if stress:
        assert not any(e["event"] == "stress_failed" for e in events), events
        finished = next(e for e in events if e["event"] == "stress_complete")
        assert finished["single_visited"] == finished["spread_visited"] == opened["count"], finished
    else:
        assert any(e["event"] == "direction" and e["rtl"] for e in events), events
        assert any(e["event"] == "smoke_complete" for e in events), events
    assert all(e.get("cache_bytes", 0) <= 192 * 1024 * 1024 for e in pages), pages
    hits = [e["request_ms"] for e in pages if e["cache_hit"]]
    return {"ready_ms": next(e["elapsed_ms"] for e in events if e["event"] == "ready"),
            "first_page_ms": pages[0]["elapsed_ms"],
            "cache_hit_median_ms": statistics.median(hits),
            "peak_rss_mib": max(rss, default=0) / 1024,
            "max_reported_cache_mib": max(e.get("cache_bytes", 0) for e in pages) / 1024**2,
            "assertions": finished["assertions"] if stress else None,
            "pages": opened["count"], "metrics": str(metrics)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("book", type=Path)
    parser.add_argument("--app", type=Path, default=Path("dist/Sua View.app/Contents/MacOS/SuaView"))
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--stress", action="store_true", help="Traverse every page and assert rapid/spread/reverse navigation")
    parser.add_argument("--output", type=Path, default=Path("local") / time.strftime("smoke-%Y%m%d-%H%M%S"))
    args = parser.parse_args()
    results = [run(args.app.resolve(), args.book.resolve(), args.output, i, args.stress) for i in range(args.runs)]
    report = {"book": str(args.book), "runs": results,
              "median_ready_ms": statistics.median(r["ready_ms"] for r in results),
              "median_first_page_ms": statistics.median(r["first_page_ms"] for r in results),
              "median_cache_hit_ms": statistics.median(r["cache_hit_median_ms"] for r in results),
              "peak_rss_mib": max(r["peak_rss_mib"] for r in results)}
    (args.output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
