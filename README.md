# Aframp Backend

Rust/Axum backend for the Aframp platform — multi-region, edge-cached, globally distributed.

---

## Global Edge-Caching & Read-Only Replicas (Issue #348)

### Architecture Overview

```
Client
  │
  ▼
CloudFront (edge PoP — <30 ms for /public/*)
  │
  ▼
Route 53 Latency Routing  ──►  us-east-1 ALB  ──►  Primary DB (RW)
                           ──►  eu-west-1 ALB  ──►  Read Replica
                           ──►  ap-southeast-1 ALB ► Read Replica
```

- CloudFront caches `/public/*` at the edge (TTL 5 min, stale-while-revalidate 60 s).
- Route 53 latency routing directs each client to the nearest regional ALB.
- Each regional gateway detects its region via the `REGION` env var and connects to the local read replica for eventual-consistency reads.
- Strong-consistency requests are routed to the primary (us-east-1) via the `X-Consistency: strong` header.

---

## Eventual vs Strong Consistency — Endpoint Mapping

| Path prefix | Consistency | Cache policy | DB target | Notes |
|---|---|---|---|---|
| `/public/*` | **Eventual** | `public, max-age=300, stale-while-revalidate=60` | Read replica | Exchange rates, fee schedules, public docs |
| `/api/v1/rates` | **Eventual** | `public, max-age=300, stale-while-revalidate=60` | Read replica | Rate feed — tolerates 5 min staleness |
| `/api/v1/fees` | **Eventual** | `public, max-age=300, stale-while-revalidate=60` | Read replica | Fee structures |
| `/account/*` | **Strong** | `no-store, private` | Primary | Balances, profile, KYC status |
| `/api/v1/onramp/*` | **Strong** | `no-store, private` | Primary | Payment initiation |
| `/api/v1/offramp/*` | **Strong** | `no-store, private` | Primary | Redemption / withdrawal |
| `/api/v1/mint/*` | **Strong** | `no-store, private` | Primary | Token minting |
| `/api/v1/transaction*` | **Strong** | `no-store, private` | Primary | Transaction history writes |
| `/api/v1/transfer*` | **Strong** | `no-store, private` | Primary | Transfers |
| `/api/v1/redemption*` | **Strong** | `no-store, private` | Primary | Redemption flow |
| `/health/edge` | N/A | `no-store` | Primary (lag check) | DNS failover probe |

### Forcing Strong Consistency

Any endpoint can be forced to the primary by sending:

```http
X-Consistency: strong
```

The gateway will:
1. Set `X-Route-Primary: true` on the response (read by the load balancer).
2. Select `DATABASE_URL` (primary) instead of `DATABASE_READ_REPLICA_URL`.

### Consistency Header Flow

```
Request  ──► Gateway middleware (edge_cache.rs)
              │
              ├─ X-Consistency: strong?
              │     YES → X-Route-Primary: true, use DATABASE_URL
              │     NO  → use DATABASE_READ_REPLICA_URL (if available)
              │
              └─ Path-based Cache-Control injected on response
```

---

## Health & Failover

`GET /health/edge` — used by Route 53 health checks.

| Condition | HTTP | DNS action |
|---|---|---|
| All dependencies healthy, lag ≤ 5 s | `200 OK` | No action |
| Replication lag > 5 s | `503` | Route 53 fails over to next region |
| Any dependency down | `503` | Route 53 fails over to next region |

Response body example:

```json
{ "status": "healthy", "region": "eu-west-1", "replication_lag_secs": 0 }
```

---

## Infrastructure

| File | Purpose |
|---|---|
| `infra/terraform/edge.tf` | CloudFront distribution + path-based cache policies |
| `infra/terraform/global_lb.tf` | Route 53 latency routing + health checks |

### Required Environment Variables (per region)

| Variable | Description |
|---|---|
| `REGION` | AWS region this instance runs in (e.g. `eu-west-1`) |
| `DATABASE_URL` | Primary PostgreSQL URL (us-east-1) |
| `DATABASE_READ_REPLICA_URL` | Local read replica URL |

---

## Latency Verification

```bash
# Run against staging
BASE_URL=https://staging-api.aframp.io ./dist-test.sh

# Run against production with regional IP overrides
US_EAST_1_IP=1.2.3.4 EU_WEST_1_IP=5.6.7.8 AP_SOUTHEAST_1_IP=9.10.11.12 \
  ./dist-test.sh https://api.aframp.io
```

Target: **< 30 ms** average for `/public/*` endpoints (cache hit at CloudFront PoP).

---

## Worker Configuration

### Stellar Confirmation Worker (#782)

| Env var | Default | Dev | Prod | Description |
|---|---|---|---|---|
| `STELLAR_CONFIRM_POLL_INTERVAL_SECS` | `15` | `1` | `5` | How often the worker polls Horizon for pending confirmations |
| `STELLAR_CONFIRM_THRESHOLD` | `1` | `1` | `1` | Minimum ledger confirmations before marking complete |
| `STELLAR_CONFIRM_STALE_TIMEOUT_SECS` | `1800` | `60` | `1800` | Transactions older than this are flagged stale |
| `STELLAR_CONFIRM_BATCH_SIZE` | `200` | `50` | `200` | Max transactions fetched per polling cycle |
| `STELLAR_CONFIRM_WINDOW_HOURS` | `48` | `1` | `48` | Look-back window for active transactions |

### Address Book Maintenance Worker (#783)

| Env var | Default | Description |
|---|---|---|
| `ADDRESS_BOOK_REVERIFY_BATCH_SIZE` | `100` | Max Stellar addresses re-verified per maintenance cycle. Lower under high Horizon load. |

### Event Bus (#784)

| Env var | Default | Description |
|---|---|---|
| `EVENT_BUS_CHANNEL_CAPACITY` | `1024` | Tokio broadcast channel buffer size. Increase if consumers log `aframp_event_bus_dropped_total` warnings. |

When a slow consumer lags behind the buffer, dropped messages are logged at `WARN` level with the count and the `aframp_event_bus_dropped_total` metric is incremented. Use `EventBus::subscribe_guarded()` instead of `subscribe()` to get lag detection automatically.
