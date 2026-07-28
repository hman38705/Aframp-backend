# Soroban / Stellar XDR SDK upgrade evaluation

Status: **blocked** — not attempted in this change. See "Why this is blocked, not just deferred" below.

## Current state (Cargo.toml)

| Crate            | Pinned version | Corresponds to  |
|------------------|----------------|-----------------|
| `soroban-sdk`    | `21.0.0`       | Protocol 21 (mainnet since 2024-06-18) |
| `stellar-xdr`    | `25.0.0`       | Protocol 25 (mainnet since 2026-01-22) |
| `stellar-strkey` | `0.0.16`       | latest published is `0.0.18` |
| `stellar_sdk`    | `0.1.4`        | latest published is also `0.1.4` (community Horizon client, not protocol-versioned) |

Current mainnet protocol as of 2026-07-27 is **Protocol 27** (since 2026-06-18). Official
Stellar version-mapping (developers.stellar.org/docs/networks/software-versions):

| Protocol | Mainnet date | Rust XDR | Soroban Rust SDK |
|----------|--------------|----------|-------------------|
| 21       | 2024-06-18   | v21.0.1  | 21.0.1-preview.3  |
| 22       | 2024-12-05   | v22.0.0  | v22.0.3           |
| 23       | 2025-09-03   | v23.0.0  | v23.0.2           |
| 24       | 2025-10-22   | 24.0.1   | 23.0.3            |
| 25       | 2026-01-22   | 25.0.0   | 25.0.0            |
| 26       | 2026-05-06   | 26.0.1   | 26.0.1            |
| 27       | 2026-06-18   | v27.0    | 27.0.2            |

Two separate problems fall out of this:

1. **`soroban-sdk` is 6 protocol versions stale** (21 → 27). This is the crate that
   compiles `src/lib.rs`'s `EscrowContract` (`#[contract]`/`#[contractimpl]`, gated
   `#[cfg(not(feature = "database"))]`) to wasm for on-chain deployment.
2. **`soroban-sdk` and `stellar-xdr` are pinned to different protocol eras** (21 vs.
   25) *right now*, independent of any upgrade. `soroban-sdk` 21.0.0's own internal
   XDR/env dependencies (`soroban-env-host`/`soroban-env-guest`) are exact-pinned to
   `=21.0.0`, while this crate separately depends on `stellar-xdr 25.0.0` directly
   (feature `database`, used in `src/chains/stellar`, `src/stellar`, etc. for
   Horizon-side XDR handling). These are disjoint dependency subgraphs today (the
   `database` feature is `no_std`-excluded from the contract build, so they don't
   collide inside a single compiled artifact), but it means "the XDR types this repo
   uses for wallet/Horizon code" and "the XDR types the on-chain contract compiles
   against" already disagree on wire format by 4 protocol versions. Any future code
   that bridges the two (e.g., decoding a contract invocation's XDR on the server
   side using the same struct shapes the contract emits) is at risk of silent
   mismatches today, before touching the SDK-27 question at all.

## What changed between the pinned version and current (22 → 27)

- **Protocol 22 ("Auth Next")**: reworked contract authorization — nonce handling
  moved from auto-incrementing to random per-signature values, and the
  authorization-entry XDR shape changed. This is **breaking for any contract that
  calls `require_auth`/`require_auth_for_args`**, which `EscrowContract` does in
  `initialize`, `set_admin`, `set_fee_rate`, `pause`, `unpause`, and (per the
  contract's purpose) presumably the order-lifecycle methods later in the file.
  Source-level call sites (`admin.require_auth()`) are unlikely to need code
  changes, but the authorization *proofs* clients submit against this contract
  change shape, and existing integration/e2e tests or SDKs that construct auth
  entries manually will need updating.
- **Protocols 23–27**: state archival / TTL extension changes, stellar-core version
  bumps, and further host-function additions across each release. I have not
  itemized these line-by-line — doing so accurately requires the released
  changelogs for `rs-soroban-env` and `rs-stellar-xdr` at each intermediate tag,
  cross-referenced against actual usage in this contract, which is exactly the
  verification step this evaluation is blocked on (see below).

## Why this is blocked, not just deferred

This session has no Rust toolchain available (`cargo`/`rustc` are not installed in
this sandbox), so none of the following — required before landing a bump this size
— could be done here:

- Compile `EscrowContract` against `soroban-sdk 27.0.2` and fix any breakage.
- Compile the `database`-feature server build against `stellar-xdr 27.0.0` /
  `stellar-strkey 0.0.18` and fix any breakage in `src/chains/stellar`,
  `src/stellar`, `src/multisig/xdr_builder.rs`, and the new
  `src/chains/stellar/xdr_parser.rs`.
- Run the integration test suite (`cargo test --tests --all-features`) end to end.
- Redeploy/re-test the escrow contract against testnet under Auth Next semantics.

Bumping the version strings in `Cargo.toml` without being able to do the above would
just push an unverified, likely-broken build onto whoever runs CI next. Given the
6-version jump crosses at least one confirmed breaking change (Auth Next) directly
affecting this contract's `require_auth` calls, that risk is not acceptable to take
blind.

## Recommended path forward (for whoever picks this up with a working toolchain)

1. Fix the pre-existing mismatch first and separately: decide whether the
   `database`-feature `stellar-xdr`/`stellar-strkey` pins should track the same
   protocol as `soroban-sdk`, and record that decision — don't let the two drift
   further apart while the SDK bump is pending.
2. Upgrade `soroban-sdk` one major/protocol version at a time (21 → 22 → 23 → 24 →
   25 → 26 → 27), running `cargo check --no-default-features` (the contract is built
   with `database` off) after each step, rather than jumping straight to 27 — this
   isolates which protocol's changes caused which compile error instead of debugging
   all of them at once.
3. Pay special attention at the 22 step to `require_auth`/`require_auth_for_args`
   call sites and any test helper that constructs `soroban_sdk::testutils::Address`
   auth entries by hand.
4. Once compiling, bump `stellar-xdr` to `27.0.0` and `stellar-strkey` to `0.0.18`
   together (`database` feature) and run the full integration suite.
5. Leave `stellar_sdk = "0.1.4"` as-is — it's already the newest version published
   for that crate; there is nothing to upgrade to.
6. Redeploy `EscrowContract` to testnet and exercise the full order lifecycle before
   considering this done — Auth Next changes the shape of authorization but not, as
   far as this evaluation could determine without a toolchain, the Rust-level call
   sites, so a compile pass alone would not be sufficient evidence of correctness.

## Sources

- [Software Versions — Stellar Docs](https://developers.stellar.org/docs/networks/software-versions)
- [soroban-sdk — crates.io](https://crates.io/crates/soroban-sdk)
- [stellar-xdr — crates.io](https://crates.io/crates/stellar-xdr)
- [stellar-strkey — crates.io](https://crates.io/crates/stellar-strkey)
- [stellar_sdk — crates.io](https://crates.io/crates/stellar_sdk)
