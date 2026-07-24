# Rate Limiting Policy

Aframp has two layers that can reject a request on rate: nginx (the edge
reverse proxy) and the application's Redis-backed rate-limit middleware.
This document is the single statement of which one owns what, so the two
never drift out of sync again.

## Single source of truth: `rate_limits.yaml`

All product/business rate-limit policy — per-endpoint, per-IP, and
per-wallet limits and windows — is defined in [`rate_limits.yaml`](../rate_limits.yaml)
at the repo root and enforced by the application middleware
(`src/middleware/rate_limit.rs`), backed by Redis sorted sets so limits are
consistent across all app instances. This is the only place product rate
limits should be changed.

Examples from that file:

- `/api/onramp/initiate`: 5 requests / wallet / hour
- `/api/auth/challenge`: 10 requests / IP / minute
- default: 100 requests / IP / minute

## Nginx: coarse DDoS filter only

Nginx (`config/nginx/nginx.conf` and the `nginx-config` ConfigMap in
`k8s/production/configmap.yaml`) applies a single `limit_req_zone` at
**10,000 requests/second per IP** (burst 2,000, `nodelay`). This exists
solely to shed volumetric/DDoS-scale abuse before it reaches the
application — it is not, and must not become, a product rate-limit policy
surface.

The nginx ceiling is deliberately set far above every limit in
`rate_limits.yaml` (the strictest of which is 5 requests/hour) so that:

- A consumer who is within the app's limits is never blocked by nginx.
- A consumer who is over the app's limits always sees the app's 429
  (with correct `Retry-After` / rate-limit headers), not nginx's generic
  one — the app is the layer responsible for user-facing rate-limit
  semantics.

If nginx's `limit_req_status 429` ever fires in practice, it indicates
DDoS-scale traffic, not a normal user hitting a product limit — treat it
as an infra/security signal, not a rate-limit policy bug.

## Changing limits

- **Product rate limits** (per-endpoint/IP/wallet quotas): edit
  `rate_limits.yaml`. No nginx change needed.
- **DDoS ceiling**: edit the `limit_req_zone` rate in both
  `config/nginx/nginx.conf` and `k8s/production/configmap.yaml` together,
  and keep it well above the highest limit in `rate_limits.yaml`.

## Metrics

- App-level rate-limit breaches: `aframp_rate_limit_breaches_total`
  (emitted by `src/middleware/rate_limit.rs`).
- Nginx-level DDoS-filter rejections: `nginx_rate_limit_rejections_total`,
  derived from the `limit_req_status` field in nginx's JSON access log via
  the Vector `log_to_metric` pipeline (`k8s/logging/vector-configmap.yaml`),
  plus standard connection/throughput metrics scraped from nginx's
  `stub_status` module by `nginx-prometheus-exporter`.
- Both are graphed on the "Nginx Rate Limiting" panel in
  `monitoring/grafana/production-operations-dashboard.json` alongside the
  app-level breach rate, so the two layers can be compared at a glance.
