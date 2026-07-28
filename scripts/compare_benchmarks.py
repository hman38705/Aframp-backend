#!/usr/bin/env python3
"""
Compare `cargo bench --output-format bencher` output against a committed
baseline and fail if any benchmark regressed by more than a threshold.

Used by the `benchmarks` CI job (.github/workflows/ci-cd.yml) to gate PRs
touching src/stellar/ or src/chains/stellar/ against performance
regressions in the zero-copy XDR parser (benches/xdr_parser.rs).

Usage:
    compare_benchmarks.py --baseline PATH --current PATH --threshold-pct N

Baseline/current files are in libtest "bencher" format, e.g.:
    test parse_envelope/tx_v1_minimal ... bench:          45 ns/iter (+/- 3)

To (re)generate the baseline locally:
    cargo bench --bench xdr_parser --features database -- --output-format bencher \\
        | tee test_snapshots/benchmarks/xdr_parser.bench.txt

Review the diff carefully before committing -- an unexpected improvement or
regression in the numbers usually means something changed in the parser,
not just machine noise.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

LINE_RE = re.compile(r"^test\s+(\S+)\s+\.\.\.\s+bench:\s*([\d,]+)\s*ns/iter")


def parse_bencher_output(text: str) -> dict[str, int]:
    results: dict[str, int] = {}
    for line in text.splitlines():
        m = LINE_RE.match(line.strip())
        if m:
            name, ns = m.group(1), int(m.group(2).replace(",", ""))
            results[name] = ns
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--current", required=True, type=Path)
    parser.add_argument("--threshold-pct", required=True, type=float)
    args = parser.parse_args()

    if not args.current.exists() or not args.current.read_text().strip():
        print(f"::error::no benchmark output found at {args.current}")
        return 1

    current = parse_bencher_output(args.current.read_text())
    if not current:
        print(f"::error::could not parse any benchmark results from {args.current}")
        return 1

    if not args.baseline.exists():
        print(
            f"::error::no baseline found at {args.baseline}. A maintainer must run "
            "the benchmarks locally (real hardware, not a shared CI runner, to keep "
            "the baseline stable) and commit the result:\n"
            "  cargo bench --bench xdr_parser --features database -- "
            f"--output-format bencher | tee {args.baseline}"
        )
        print("\nCurrent run's results, for reference:")
        for name, ns in sorted(current.items()):
            print(f"  {name}: {ns} ns/iter")
        return 2

    baseline = parse_bencher_output(args.baseline.read_text())
    if not baseline:
        print(f"::error::could not parse any benchmark results from {args.baseline}")
        return 1

    failures = []
    for name, current_ns in sorted(current.items()):
        baseline_ns = baseline.get(name)
        if baseline_ns is None:
            print(f"::notice::'{name}' has no baseline entry yet (new benchmark)")
            continue
        allowed = baseline_ns * (1 + args.threshold_pct / 100)
        delta_pct = (current_ns - baseline_ns) / baseline_ns * 100
        status = "REGRESSION" if current_ns > allowed else "OK"
        if status == "REGRESSION":
            failures.append((name, baseline_ns, current_ns, delta_pct))
        print(
            f"  {name}: {current_ns} ns/iter (baseline {baseline_ns} ns/iter, "
            f"{delta_pct:+.1f}%) [{status}]"
        )

    if failures:
        print(f"\n::error::{len(failures)} benchmark(s) regressed by more than {args.threshold_pct}%:")
        for name, baseline_ns, current_ns, delta_pct in failures:
            print(f"  - {name}: {baseline_ns} -> {current_ns} ns/iter ({delta_pct:+.1f}%)")
        return 1

    print(f"\nAll benchmarks within {args.threshold_pct}% of baseline.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
