# Aframp

**Building the POS network for Stellar in Africa.**

Aframp brings Stellar-powered payments into everyday physical commerce, starting in Nigeria. The idea is simple: Nigerians already understand the POS terminal — tap, transfer, withdraw. Aframp adds another familiar option on top of that muscle memory: **scan and pay**, settled on Stellar, without the merchant or customer ever needing to think about wallets, addresses, or blockchains.

```
Customer → Scan → Pay → Confirmed
```

From the merchant's point of view, that's the whole product. Stellar is the settlement layer underneath; Aframp is the experience on top.

## Why

Cross-border commerce in Africa is fragmented across national payment systems — multiple currencies, high fees, slow settlement, and merchant onboarding that doesn't travel across borders. Stablecoins and blockchain rails already move value globally, fast and cheap. What's missing is the everyday, physical-commerce bridge between the two. Aframp aims to be that bridge: onboard merchants first, and let consumer demand for Stellar wallets follow.

Longer-term shape of the platform (not all built yet — see [Status](#status) below):

| Layer | Purpose |
|---|---|
| **Aframp Pay** | Merchant-facing payments: requests, QR codes, receive Stellar payments, receipts, revenue tracking |
| **Aframp Wallet** | Consumer wallet built for spending, not just holding |
| **Aframp Business** | Merchant dashboard: analytics, reconciliation, multi-location, invoices |
| **Aframp API** | Infrastructure layer for other African fintechs to integrate Stellar payments |

This repository is the backend underneath **Aframp Pay**.

## Status: MVP in progress

The MVP is scoped to seven things: merchant accounts, payment request generation, QR-based payment, Stellar transaction creation, transaction monitoring, payment confirmation, and merchant transaction history. Here's what's live today:

| MVP capability | Status |
|---|---|
| Merchant accounts (signup/login) | ✅ Done |
| Payment request generation | 🚧 Not yet started |
| QR-based payment | 🚧 Not yet started |
| Stellar transaction creation | 🚧 `StellarClient` scaffolded, not implemented |
| Transaction monitoring | 🚧 Polling worker scaffolded, detection logic not implemented |
| Payment confirmation | 🚧 Schema supports the `detected → verified → confirmed` pipeline; nothing populates it yet |
| Merchant transaction history | ✅ Done |

Also implemented, one layer ahead of the MVP list: fiat withdrawals (cash out a cNGN balance to a bank account) and merchant API keys (test/live).

## How it will work

A merchant enters an amount — say ₦10,000 — and Aframp generates a payment request and QR code. A customer scans it with a Stellar-compatible wallet and pays. Aframp watches the Stellar network, detects and verifies the transaction, and confirms it back to the merchant. The merchant never sees a blockchain explorer; they see "Payment received."

Today, in this codebase, that maps to:

- A merchant is created via `/signup` and gets a **Stellar wallet** address (`/wallet/create`).
- Off-ramp deposits (customer → merchant, in cNGN on Stellar) are meant to be picked up by a background worker polling [Stellar Horizon](https://developers.stellar.org/docs/data/horizon), correlated to a merchant via memo, and moved through a `detected → verified → confirmed` pipeline into the merchant's `balance`.
- Merchants can withdraw their available cNGN balance to a Nigerian bank account (`/withdraw`), which is where fiat actually leaves the system.

## Tech stack

- **Rust** + [Axum](https://github.com/tokio-rs/axum) — HTTP API
- **PostgreSQL** via [sqlx](https://github.com/launchbadge/sqlx) — compile-time-checked queries
- **Stellar** ([Horizon](https://developers.stellar.org/docs/data/horizon)) — settlement network, cNGN as the initial asset
- **JWT** ([jsonwebtoken](https://github.com/Keats/jsonwebtoken)) + **Argon2** — auth and password hashing
- **Tokio** — async runtime, including the background Stellar polling worker

## Getting started

### Prerequisites

- Rust (stable, 2021 edition)
- PostgreSQL (local or remote)

### Setup

```bash
cp .env.example .env
```

Fill in `.env`:

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | yes | — | Postgres connection string |
| `APP_BIND_ADDR` | no | `127.0.0.1:3000` | Address the HTTP server binds to |
| `JWT_SECRET` | yes | — | Secret used to sign merchant session tokens |
| `WEBHOOK_SECRET` | yes | — | Secret used to verify inbound provider webhooks |
| `STELLAR_SYSTEM_WALLET_ADDRESS` | yes | — | The Stellar account Aframp watches for incoming deposits |
| `STELLAR_HORIZON_URL` | no | `https://horizon-testnet.stellar.org` | Horizon endpoint to poll |
| `STELLAR_POLL_INTERVAL_SECS` | no | `60` | How often the confirmation worker polls Horizon |

Run migrations and start the server:

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
sqlx database create
sqlx migrate run

cargo run
```

The server starts on `APP_BIND_ADDR` (default `http://127.0.0.1:3000`) and spawns the Stellar confirmation worker in the background.

### Running tests

Integration tests need a separate database and are skipped automatically if it isn't configured:

```bash
export TEST_DATABASE_URL=postgres://postgres:postgres@localhost/aframp_test
sqlx database create --database-url "$TEST_DATABASE_URL"
cargo test
```

## API reference

All authenticated routes expect `Authorization: Bearer <token>`, where `<token>` is the JWT returned from `/signup` or `/login`.

| Method | Path | Auth | Description |
|---|---|---|---|
| `POST` | `/signup` | — | Create a user + merchant account. Body: `{ email, password (min 8 chars), name }` |
| `POST` | `/login` | — | Authenticate. Body: `{ email, password }` |
| `POST` | `/wallet/create` | ✅ | Create a Stellar wallet for the authenticated merchant. Body: `{ network? }` (defaults to `stellar`) |
| `GET` | `/wallet` | ✅ | Get the merchant's wallet |
| `GET` | `/balance` | ✅ | List balances by asset |
| `GET` | `/transactions?limit=` | ✅ | List the merchant's payments (default limit 50, max 200) |
| `POST` | `/withdraw` | ✅ | Withdraw available balance to a bank account. Body: `{ amount_stroops, asset?, bank_code, account_number }` |
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
  blockchain/  Background worker that polls Stellar Horizon
  stellar/     Stellar client (deposit detection, memo correlation)
  models/      Request/response and row types
  services/    Business logic (users, wallets, balances, payments, withdrawals)
  payments/    Pluggable payment provider abstraction (mock provider for now)
migrations/    SQL schema migrations (sqlx)
tests/         Integration tests (auth, wallet, withdrawal flows)
```

## Why Nigeria first

Nigeria has a large digital-payments ecosystem and near-universal familiarity with POS and bank-transfer payments — the exact behavior Aframp is extending rather than replacing. The plan is to prove the merchant payment experience narrowly here, then expand to other African markets and cross-border corridors.

## Contributing

This project is under active MVP development — expect the API and schema to change as payment requests, QR flows, and Stellar transaction confirmation land. Open an issue or PR against `master`.
