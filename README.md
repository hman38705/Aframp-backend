# Aframp

**Building the POS network for Stellar in Africa.**

Aframp brings Stellar-powered payments into everyday physical commerce, starting in Nigeria. The idea is simple: Nigerians already understand the POS terminal — tap, transfer, withdraw. Aframp adds another familiar option on top of that muscle memory: **scan and pay**, settled on Stellar, without the merchant or customer ever needing to think about wallets, addresses, or blockchains.

```
Customer → Scan → Pay → Confirmed
```

From the merchant's point of view, that's the whole product. Stellar is the settlement layer underneath; Aframp is the experience on top.

## Why

Cross-border commerce in Africa is fragmented across national payment systems — multiple currencies, high fees, slow settlement, and merchant onboarding that doesn't travel across borders. Stablecoins and blockchain rails already move value globally, fast and cheap. What's missing is the everyday, physical-commerce bridge between the two. Aframp aims to be that bridge: onboard merchants first, and let consumer demand for Stellar wallets follow.

Longer-term shape of the platform (not all built yet — see [Status](#status-real-progress-not-aspiration) below):

| Layer | Purpose |
|---|---|
| **Aframp Pay** | Merchant-facing payments: requests, QR codes, receive Stellar payments, receipts, revenue tracking |
| **Aframp Wallet** | Consumer wallet built for spending, not just holding |
| **Aframp Business** | Merchant dashboard: analytics, reconciliation, multi-location, invoices |
| **Aframp API** | Infrastructure layer for other African fintechs to integrate Stellar payments |

This repository is the backend underneath **Aframp Pay**.

## Status: real progress, not aspiration

This section is deliberately literal: everything marked ✅ has been exercised end-to-end against a real Postgres database and, where it touches Stellar, a real testnet transaction — not asserted from reading the code. Everything marked 🚧 is a genuine gap, described honestly rather than smoothed over.

### What's real today

| Capability | Status | Proof |
|---|---|---|
| Merchant signup / login | ✅ Done | Argon2-hashed passwords, real Postgres rows, real HMAC-signed JWTs |
| Every money/data endpoint requires auth | ✅ Done | `/wallet`, `/balance`, `/transactions`, `/withdraw*` all return `401` with no token or a garbage token — enforced at the type level via Axum's `AuthUser` extractor, not an ad hoc check |
| Stellar wallet generation | ✅ Done | Each `/wallet/create` call generates a **real ed25519 keypair**, encoded as a genuine Stellar `G...` address — not a placeholder string. Verified against Horizon directly. |
| Wallet key custody | ✅ Done | The private key (`S...` seed) is AES-256-GCM encrypted before it ever touches the database, keyed by `WALLET_ENCRYPTION_KEY`. Confirmed the API response never includes it, and the DB column holds ciphertext, not plaintext. |
| Stellar deposit detection | ✅ Done | Background worker polls Horizon's payments feed per merchant wallet, on a timer, handling both `create_account` (how a brand-new wallet is always first funded on Stellar) and regular `payment` ops. Proven with a real testnet transaction: funded a generated wallet via Stellar's friendbot, and watched the exact transaction hash and amount land in `/transactions` and `/balance` within one poll cycle. |
| Balance ledger | ✅ Done | `/balance` reflects real detected deposits, not a stub — because the above is real, this is real too. |
| Merchant transaction history | ✅ Done | `/transactions` lists real detected payments |
| Withdrawal request + ledger accounting | ✅ Done | `/withdraw` atomically debits `available` balance and records the request; insufficient-balance and validation checks are enforced in the same DB transaction |

### What's still a stub or missing

| Gap | What's actually there today |
|---|---|
| Payment request generation | Not started — no endpoint to generate a specific amount + QR payload yet |
| QR-based payment | Not started |
| Real payout provider | `/withdraw` debits the internal ledger and parks a `pending` row, but nothing calls a bank/payment rail — no money actually leaves the system. A `PaymentProvider` trait and `MockProvider` exist in `src/payments/` but aren't wired into the withdrawal flow at all (dead code today) |
| Confirmation-depth threshold | Deposits move `detected → verified → confirmed` immediately on detection — there's no real "wait N ledger confirmations" logic yet (Stellar has fast finality, so this matters less than on Bitcoin, but it's still an open TODO in `blockchain/worker.rs`) |
| Settlement/sweep wallet | Each merchant's Stellar secret is held (encrypted) by the platform, but nothing yet sweeps funds from individual merchant wallets into a platform settlement wallet. `STELLAR_SYSTEM_WALLET_ADDRESS` is still validated at startup and reserved for this, but isn't used by anything yet |
| `src/stellar/mod.rs` | Vestigial stub from an earlier, abandoned design (single system wallet + memo-based correlation). Not compiled into the binary's active module tree in any meaningful way, superseded by the per-wallet design in `src/blockchain/`. Left in place as known cleanup debt rather than silently deleted. |

## How it actually works today

- A merchant signs up (`/signup`) and creates a wallet (`/wallet/create`), which generates a real Stellar keypair. The public address is returned; the private key is encrypted and stored server-side — this is a **custodial** design, not "bring your own wallet."
- A background worker polls Horizon for every merchant wallet's address on a timer (`STELLAR_POLL_INTERVAL_SECS`), detects incoming payments, and moves them through Postgres into that merchant's balance.
- Merchants can withdraw their available balance (`/withdraw`) to a Nigerian bank account — today this only updates internal bookkeeping; the actual fiat payout leg isn't connected yet (see gaps above).

The originally-planned memo-based correlation model (one shared system wallet, deposits routed by transaction memo) was replaced with real per-merchant wallets during development — simpler to reason about and matches how a QR-code-per-merchant product actually needs to work.

## Tech stack

- **Rust** + [Axum](https://github.com/tokio-rs/axum) — HTTP API
- **PostgreSQL** via [sqlx](https://github.com/launchbadge/sqlx) — runtime-checked queries
- **Stellar** ([Horizon](https://developers.stellar.org/docs/data/horizon)) — settlement network; deposit detection via [reqwest](https://github.com/seanmonstar/reqwest)
- **ed25519-dalek** + **stellar-strkey** — real Stellar keypair generation
- **aes-gcm** — encrypts wallet private keys at rest
- **JWT** ([jsonwebtoken](https://github.com/Keats/jsonwebtoken)) + **Argon2** — auth and password hashing
- **Tokio** — async runtime, including the background Stellar polling worker

## Getting started

### Prerequisites

- Rust (stable, 2021 edition)
- PostgreSQL — either a local instance, or Docker (see below; this is the path actually verified during development on a machine where the local Postgres cluster wasn't running)

### Setup

```bash
cp .env.example .env
```

Fill in `.env`:

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | yes | — | Postgres connection string |
| `APP_BIND_ADDR` | no | `127.0.0.1:3000` | Address the HTTP server binds to |
| `JWT_SECRET` | yes | — | Secret used to sign merchant session tokens. Generate with `openssl rand -hex 32` |
| `WEBHOOK_SECRET` | yes | — | Secret used to verify inbound provider webhooks. Generate with `openssl rand -hex 32` |
| `WALLET_ENCRYPTION_KEY` | yes | — | AES-256-GCM key encrypting Stellar wallet secrets at rest. Generate with `openssl rand -hex 32` (must decode to exactly 32 bytes) |
| `STELLAR_SYSTEM_WALLET_ADDRESS` | yes | — | Reserved for a future platform settlement/sweep wallet. Validated at startup but not used by deposit detection today (see [Status](#status-real-progress-not-aspiration)) |
| `STELLAR_HORIZON_URL` | no | `https://horizon-testnet.stellar.org` | Horizon endpoint to poll |
| `STELLAR_POLL_INTERVAL_SECS` | no | `60` | How often the deposit-detection worker polls Horizon, per wallet |

### Quick start (Docker Postgres)

```bash
docker run -d --name aframp-postgres \
  -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=aframp \
  -p 5432:5432 postgres:16

docker exec -i aframp-postgres psql -U postgres -d aframp < migrations/0001_init.sql
docker exec -i aframp-postgres psql -U postgres -d aframp < migrations/0002_wallet_secret_key.sql

cargo run
```

### Alternative: sqlx-cli against an existing Postgres

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
sqlx database create
sqlx migrate run

cargo run
```

The server starts on `APP_BIND_ADDR` (default `http://127.0.0.1:3000`) and spawns the Stellar deposit-detection worker in the background.

See **[`command.txt`](command.txt)** for a copy-paste reference of every command used to run, test, and interact with this backend — curl calls for every endpoint, secret generation, and DB lifecycle commands.

### Running tests

Integration tests need a separate database, and **silently skip with a false "ok" if it isn't configured** — this bit us during development (a full green `cargo test` run had actually tested nothing). Always set `TEST_DATABASE_URL` before trusting the result:

```bash
docker exec -i aframp-postgres psql -U postgres -c "CREATE DATABASE aframp_test;"
TEST_DATABASE_URL=postgres://postgres:postgres@localhost:5432/aframp_test cargo test
```

## API reference

All authenticated routes expect `Authorization: Bearer <token>`, where `<token>` is the JWT returned from `/signup` or `/login`.

| Method | Path | Auth | Description |
|---|---|---|---|
| `POST` | `/signup` | — | Create a user + merchant account. Body: `{ email, password (min 8 chars), name }` |
| `POST` | `/login` | — | Authenticate. Body: `{ email, password }` |
| `POST` | `/wallet/create` | ✅ | Generate a real Stellar wallet for the authenticated merchant. Body: `{ network? }` (defaults to `stellar`) |
| `GET` | `/wallet` | ✅ | Get the merchant's wallet |
| `GET` | `/balance` | ✅ | List balances by asset, reflecting real detected Stellar deposits |
| `GET` | `/transactions?limit=` | ✅ | List the merchant's detected payments (default limit 50, max 200) |
| `POST` | `/withdraw` | ✅ | Debit available balance and record a withdrawal request. Body: `{ amount_stroops, asset?, bank_code, account_number }`. **Note:** no real payout happens yet — see [Status](#status-real-progress-not-aspiration) |
| `GET` | `/withdrawals?limit=` | ✅ | List the merchant's withdrawals |
| `GET` | `/health` | — | Liveness check (`204 No Content`) |

`/signup` and `/login` both return:

```json
{ "token": "...", "user_id": "...", "merchant_id": "..." }
```

## Project layout

```
src/
  api/         HTTP handlers (thin — validation + calling services)
  auth/        JWT signing/verification, password hashing, auth extractor
  blockchain/  Stellar integration: keypair generation, wallet-secret encryption,
               Horizon deposit polling, and the background worker that drives it
  models/      Request/response and row types
  services/    Business logic (users, wallets, balances, payments, withdrawals)
  payments/    Pluggable payment provider abstraction (mock provider — not wired in yet)
  stellar/     Vestigial unused stub from an earlier design — see Status
migrations/    SQL schema migrations (sqlx)
tests/         Integration tests (auth, wallet, withdrawal flows)
command.txt    Copy-paste command reference for running/testing/interacting with the backend
```

## Why Nigeria first

Nigeria has a large digital-payments ecosystem and near-universal familiarity with POS and bank-transfer payments — the exact behavior Aframp is extending rather than replacing. The plan is to prove the merchant payment experience narrowly here, then expand to other African markets and cross-border corridors.

## Contributing

This project is under active MVP development — expect the API and schema to change as payment requests, QR flows, real payout provider integration, and settlement sweeping land. Open an issue or PR against `master`.
