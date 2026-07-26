# Aframp SLO Definitions

Closes #788. Companion to `monitoring/grafana/slo-dashboard.json` and the
`aframp.slo` Prometheus alert group in `monitoring/prometheus/rules/aframp_alerts.yml`.

## Service Level Objectives

| SLO | Target | Measurement window |
|-----|--------|--------------------|
| Availability | 99.9% (43.8 min/month downtime budget) | Rolling 30 days |
| P99 Latency | ≤ 500 ms | Rolling 5 minutes |
| Error Rate | ≤ 0.1% (1 in 1 000 requests) | Rolling 30 days |

## Error Budget

With a 0.1% error rate SLO over a 30-day month:

- Total requests at peak: varies
- Monthly error budget: **43.8 minutes** of 100% outage, or proportional partial errors

## Burn Rate Thresholds

| Window | Multiplier | Severity | Action |
|--------|-----------|----------|--------|
| 1 h | > 2× | Critical | Page on-call immediately |
| 6 h | > 2× (confirmed) | Critical | Escalate, begin incident response |
| 24 h | > 1.1× | Warning | Investigate within 4 hours |
| 72 h | > 1.0× | Info | Review in next engineering sync |

A burn rate of **2×** means the monthly budget is consumed in 15 days.
A burn rate of **14.4×** means the monthly budget is consumed in 2 hours (fast-burn page threshold).

## Peg Integrity SLO (#786)

| Metric | Threshold | Severity |
|--------|-----------|----------|
| CNGN supply / reserves ratio | > 1.00001 (0.001%) | WARNING |
| CNGN supply / reserves ratio | > 1.0001 (0.01%) | CRITICAL — page on-call |
| Reserve data staleness | > 2 hours | WARNING |

Prometheus recording rule: `aframp:cngn_reserve_ratio:hourly`

Runbooks:
- Critical: https://runbooks.aframp.io/mint-reserve-ratio-critical
- Warning: https://runbooks.aframp.io/mint-reserve-ratio-warning
- SLO fast burn: https://runbooks.aframp.io/slo-fast-burn
- SLO slow burn: https://runbooks.aframp.io/slo-slow-burn
- P99 latency: https://runbooks.aframp.io/slo-latency-violation
