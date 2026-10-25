#!/usr/bin/env python3
"""Measure the current release scanner. No filesystem contents are read by this wrapper."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('path', type=Path)
parser.add_argument('--runs', type=int, default=3)
parser.add_argument('--binary', type=Path, default=Path(__file__).resolve().parents[1] / 'target/release/disk-lens')
args = parser.parse_args()
if args.runs < 1:
    parser.error('--runs must be positive')
binary = args.binary.resolve()
if not binary.is_file():
    parser.error('Build the release binary first: cargo build --locked --release')
rows = []
for _ in range(args.runs):
    start = time.perf_counter()
    result = subprocess.run(['/usr/bin/time', '-l', str(binary), '--scan', str(args.path.resolve())], text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stderr or result.stdout)
    metrics = dict(re.findall(r'(logical|allocated|files|errors|seconds)=([\d.]+)', result.stdout))
    if set(metrics) != {'logical', 'allocated', 'files', 'errors', 'seconds'}:
        raise SystemExit('Unexpected scanner output: ' + result.stdout)
    rss = re.search(r'(\d+)\s+maximum resident set size', result.stderr)
    rows.append({'wall_seconds': time.perf_counter() - start, 'scan_seconds': float(metrics.pop('seconds')),
                 'peak_resident_bytes': int(rss[1]) if rss else None, **{k: int(v) for k, v in metrics.items()}})
report = {'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'path': str(args.path.resolve()),
          'workers_override': os.environ.get('DISK_LENS_SCAN_WORKERS'), 'buffer_override': os.environ.get('DISK_LENS_BULK_BYTES'),
          'method': 'Sequential release CLI runs on this Mac; no cache reset, GUI excluded.', 'runs': rows,
          'median_scan_seconds': statistics.median(r['scan_seconds'] for r in rows),
          'identical_accounting': len({tuple(r[k] for k in ['logical', 'allocated', 'files', 'errors']) for r in rows}) == 1}
print(json.dumps(report, indent=2, ensure_ascii=False))
