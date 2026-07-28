# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Evaluated, blocked

- **Soroban SDK upgrade (`soroban-sdk` 21.0.0 → 27.0.2, `stellar-xdr` 25.0.0 →
  27.0.0, `stellar-strkey` 0.0.16 → 0.0.18)**: evaluated but not performed. The
  pinned `soroban-sdk` is six protocol versions behind current mainnet
  (Protocol 27, live since 2026-06-18) and is already on a different protocol
  era than the separately-pinned `stellar-xdr`. Protocol 22 ("Auth Next")
  changed contract authorization in a way that affects this repo's
  `EscrowContract` (`src/lib.rs`, `require_auth` call sites). A safe upgrade
  needs a Rust toolchain to compile and test each intermediate version and
  testnet access to re-verify the contract under the new auth semantics —
  neither was available in the environment this evaluation ran in, so the
  version bump was not attempted rather than land unverified. Full findings
  and a recommended step-by-step upgrade path:
  see [`docs/soroban-sdk-upgrade-evaluation.md`](docs/soroban-sdk-upgrade-evaluation.md).

### Added

- CI now runs the `benches/xdr_parser.rs` Criterion benchmarks on every PR
  touching `src/stellar/`, `src/chains/stellar/`, or the benchmark itself, and
  fails the build if any benchmark regresses by more than 10% against the
  committed baseline in `test_snapshots/benchmarks/` (see
  `.github/workflows/ci-cd.yml`'s `benchmarks` job and
  `scripts/compare_benchmarks.py`). Baseline generation is documented in
  `test_snapshots/benchmarks/README.md`; the job only warns, rather than
  failing the build, until that baseline is committed.
- `src/chains/stellar/xdr_parser.rs`: a zero-copy parser for the header
  fields of a `TransactionV1Envelope` (envelope type, source account, fee,
  sequence number, preconditions, memo), backing the above benchmark. The
  benchmark file referenced this module and a `Bitmesh_backend` crate alias
  before this change; neither existed, so `cargo bench` could not previously
  compile.
