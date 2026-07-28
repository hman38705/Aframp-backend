# Benchmark baselines

This directory holds committed `cargo bench --output-format bencher` output
used as the performance baseline for the `benchmarks` CI job in
`.github/workflows/ci-cd.yml`. It runs on every pull request touching
`src/stellar/`, `src/chains/stellar/`, or `benches/xdr_parser.rs`, and fails
the build if any benchmark regresses by more than 10% relative to the
committed baseline (`scripts/compare_benchmarks.py`).

## Creating or updating a baseline

Benchmarks are timing-sensitive, so baselines must be generated on quiet,
dedicated hardware — not a shared CI runner, whose noisy-neighbor variance
would make the 10% threshold meaningless in either direction. Generate
locally:

```bash
cargo bench --bench xdr_parser --features database -- --output-format bencher \
  | tee test_snapshots/benchmarks/xdr_parser.bench.txt
```

Review the diff before committing — a large unexplained swing usually means
a regression (or noise from a busy machine), not license to relax the
baseline.

## Bootstrap status

`xdr_parser.bench.txt` has not been generated yet: it must be produced by
running the command above on real hardware and committed once, after which
the `benchmarks` CI job will compare future PRs against it and start
enforcing the 10% threshold. Until then, the CI job only warns (it does not
fail the build) when it can't find a baseline.
