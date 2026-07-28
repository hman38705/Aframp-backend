# Changelog

All notable changes to this project are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions align with the `version` field in `Cargo.toml`.

> **PR requirement**: every pull request that changes `src/`, `migrations/`,
> `.github/workflows/`, or `Cargo.toml` **must** include an entry in the
> `[Unreleased]` section below. The CI `changelog-check` job enforces this on
> every PR targeting `develop` or `main`.

---

## [Unreleased]

---

## [0.27.1] — 2027-07-27

### Added
- `CHANGELOG.md` — single source of truth for release history (issue #821).
- `RecurringPaymentScheduler` service in `src/recurring/mod.rs` — full
  implementation of recurring payment scheduling: create / update / cancel
  schedules, worker dispatch, failure handling with auto-suspension, and
  consumer notifications (issue #822).
- CI `changelog-check` job in `ci-cd.yml` — fails PRs that do not update
  `CHANGELOG.md`.

---

## [0.27.0] — 2027-06-30

### Added
- **Automated DB Maintenance & Partitioning**
  (`migrations/20270630000000_automated_maintenance_partitioning.sql`) —
  declarative time-series partitioning for `risk_exposure_snapshots` and
  `partner_performance_logs`; pg_cron automated daily partition creation;
  cold-storage migration for data > 90 days; concurrent reindex helpers;
  monitoring views `v_partition_health`, `v_autovacuum_activity`.
- **Financial Reconciliation & Circuit Breakers**
  (`migrations/20270630000001_financial_reconciliation.sql`,
  `src/workers/reconciliation.rs`) — hourly reconciliation worker; 7-decimal
  precision balance tracking; on-chain Stellar verification; automated
  circuit-breaker for 5 default corridors; response time < 500 ms.
- **Rust Memory Profiling** (`src/profiling/mod.rs`, `src/allocator.rs`) —
  real-time heap tracking, allocation hot-spot detection, REST API
  (`/profiling/*`), jemalloc / mimalloc support, zero overhead when disabled.
- **Partner commission engine**
  (`migrations/20270602000001_partner_commission_engine.sql`).
- **Cache invalidation logs**
  (`migrations/20270602000000_cache_invalidation_logs.sql`).

### Changed
- `Cargo.toml` — jemalloc and mimalloc added as optional allocator features.

---

## [0.26.2] — 2027-05-29

### Added
- **DeFi Analytics Dashboard**
  (`migrations/20270529000000_defi_analytics_dashboard.sql`).
- **Mint Authorization Framework**
  (`migrations/20270528000000_mint_authorization_framework.sql`).
- **AML ML Optimization** (`migrations/20270527000000_aml_ml_optimization.sql`).
- **Sandbox Environment** (`migrations/20270527000001_sandbox_environment.sql`).
- **Performance Profiling Schema**
  (`migrations/20270503000000_performance_profiling.sql`).
- **Bank Integrations** (`migrations/20270502000000_bank_integrations.sql`).

---

## [0.26.1] — 2027-05-01

### Added
- **Cryptographic Proof-of-Reserves**
  (`migrations/20270501000000_cryptographic_por.sql`).
- **Multi-tenant Rate Limiting**
  (`migrations/20270501000001_multi_tenant_rate_limiting.sql`).
- **Multi-chain Settlement**
  (`migrations/20270501000002_multi_chain_settlement.sql`).
- **BFT Oracle Price Feeds**
  (`migrations/20270501000003_bft_oracle_price_feeds.sql`).
- **Predictive Liquidity ML**
  (`migrations/20270501000005_predictive_liquidity_ml.sql`).
- **PEP Screening Extended**
  (`migrations/20270501000004_pep_screening_extended.sql`).

---

## [0.26.0] — 2027-04-29

### Added
- **PEP Screening Engine**
  (`migrations/20270429000001_pep_screening_engine.sql`).
- **Address Book / Beneficiary Management**
  (`migrations/20270429000000_address_book_beneficiary_management.sql`,
  `src/wallet/address_book/`).
- **Travel Rule v1**
  (`migrations/20270428000003_travel_rule_schema.sql`).
- **Disaster Recovery / BCP**
  (`migrations/20270428000001_dr_bcp_schema.sql`).
- **Collateral Lending**
  (`migrations/20270428000000_collateral_lending_schema.sql`).
- **Banking Partner Integration**
  (`migrations/20270427000000_banking_partner_integration.sql`,
  `migrations/20270427000001_partner_integration_framework.sql`).
- **DeFi Risk Monitoring & Yield Compliance**
  (`migrations/20270425000000_defi_risk_monitoring_yield_compliance.sql`).
- **Append-Only Audit Ledger**
  (`migrations/20270424000000_append_only_audit_ledger.sql`,
  `src/audit/`).
- **DeFi Integration Architecture**
  (`migrations/20270415000000_defi_integration_architecture.sql`).

---

## [0.25.0] — 2027-04-03

### Added
- **Agent Swarm** (`migrations/20270403000000_agent_swarm_schema.sql`).
- **Agent Dashboard** (`migrations/20270402000000_agent_dashboard_schema.sql`).
- **Dispute Resolution**
  (`migrations/20270401000002_dispute_resolution_schema.sql`).
- **Autonomous Bargaining Protocol**
  (`migrations/20270401000001_autonomous_bargaining_protocol.sql`).
- **Agent CFO** (`migrations/20270401000000_agent_cfo_schema.sql`).

---

## [0.24.0] — 2027-03-01

### Added
- **POS / QR Payment System**
  (`migrations/20270301000000_pos_qr_payment_system.sql`,
  `src/pos/`) — SEP-7 compliant QR generation (< 300 ms), WebSocket lobby
  service (merchant notification < 3 s), legacy POS bridge (Odoo, Revel,
  Square), offline proof-of-payment with HMAC-SHA256.

---

## [0.23.0] — 2027-02-04

### Added
- **Wallet Provisioning**
  (`migrations/20270204000000_wallet_provisioning_schema.sql`,
  `src/wallet_provisioning/`).
- **Multi-store Franchise**
  (`migrations/20270203000000_multi_store_franchise_schema.sql`).
- **Merchant Invoicing & Tax**
  (`migrations/20270202000000_merchant_invoicing_tax_schema.sql`).
- **Multisig Governance**
  (`migrations/20270201000001_multisig_governance.sql`,
  `src/multisig/`).
- **Wallet Architecture V2**
  (`migrations/20270201000002_wallet_architecture.sql`).
- **Merchant CRM**
  (`migrations/20270201000000_merchant_crm_schema.sql`).

---

## [0.22.0] — 2027-01-01

### Added
- **Mint Signer Management**
  (`migrations/20270101000000_mint_signer_management.sql`).
- **Auditor Portal**
  (`migrations/20261401000000_auditor_portal_schema.sql`).
- **Pentest / Security Framework**
  (`migrations/20261301000000_pentest_security_framework.sql`,
  `src/pentest/`).
- **Circuit Breaker System Status**
  (`migrations/20261229000000_circuit_breaker_system_status.sql`).
- **Mint Requests Schema**
  (`migrations/20261220000000_mint_requests.sql`).

---

## [0.21.0] — 2026-12-15

### Added
- **Redemption Flow**
  (`migrations/20261215000001_redemption_flow_schema.sql`,
  `src/` redemption handlers).
- **Collateral Verification**
  (`migrations/20261215000000_collateral_verification.sql`).
- **API Audit Log** (`migrations/20261210000000_api_audit_log_schema.sql`).
- **Developer Portal**
  (`migrations/20261201000000_developer_portal_schema.sql`,
  `src/developer_portal/`).
- **Admin Access Control**
  (`migrations/20261101000000_admin_access_control_schema.sql`).
- **Settlement Schema**
  (`migrations/20261001000000_create_settlement_schema.sql`,
  `src/settlement/`).

---

## [0.20.0] — 2026-06-27

### Added
- **High-Throughput Stellar Submission Engine** (`src/chains/stellar/`) —
  multi-channel account pooling, lock-free sequence coordinator, dynamic fee
  engine, 50+ TPS capacity, exponential backoff retry, Prometheus metrics.
  (`migrations/20260627010000_stellar_throughput_optimization.sql`,
  `migrations/20260601000013_stellar_submission_channels.sql`).
- **AML Program Effectiveness Metrics**
  (`migrations/20260627000000_aml_program_effectiveness_metrics.sql`).

---

## [0.19.0] — 2026-06-01

### Added
- **Cluster Node Heartbeats**
  (`migrations/20260601000000_cluster_node_heartbeats.sql`).
- **CBDC Gateways & Swap Records**
  (`migrations/20260601000001_create_cbdc_gateways.sql`,
  `migrations/20260601000001500_create_cbdc_swap_records.sql`,
  `migrations/20260601000001600_create_cbdc_2pc_locks.sql`,
  `migrations/20260601000001700_create_cryptographic_signatory_vault.sql`).
- **Compliance Oracle**
  (`migrations/20260601000002_compliance_oracle_schema.sql`).
- **Database Scaling Shard Registry**
  (`migrations/20260601000006_database_scaling_shard_registry.sql`).
- **Flash Liquidity**
  (`migrations/20260601000008_flash_liquidity_schema.sql`).
- **SOR Rebalancing**
  (`migrations/20260601000010_sor_rebalancing_schema.sql`).
- **Global Edge Caching & Read Replicas** (Issue #348) — CloudFront TTL-5min
  caching for `/public/*`; Route 53 latency routing; per-region read-replica
  selection; `X-Consistency: strong` header override; `/health/edge` failover
  probe; Terraform in `infra/terraform/edge.tf` and `global_lb.tf`.

---

## [0.18.0] — 2026-05-30

### Added
- **SAR Full Schema** (`migrations/20260528000000_sar_full_schema.sql`).
- **Regulatory Evidence Package**
  (`migrations/20260529000000_regulatory_evidence_package.sql`).
- **AML Case Records**
  (`migrations/20260530000000_aml_case_records.sql`).
- **Reconciliation Tables**
  (`migrations/20260527000002_create_reconciliation_tables.sql`).
- **Audit Replication Log**
  (`migrations/20260527000001_create_audit_replication_log.sql`).
- **Compliance Alerts**
  (`migrations/20260527000000_create_compliance_alerts.sql`).
- **Consumer ID on Transactions**
  (`migrations/20260526000000_add_consumer_id_to_transactions.sql`).

---

## [0.17.0] — 2026-05-01

### Added
- **Mint & Burn Event Monitoring**
  (`migrations/20260501000000_mint_burn_event_monitoring.sql`).
- **LP Onboarding**
  (`migrations/20260501100000_lp_onboarding_schema.sql`, `src/lp_onboarding/`).

---

## [0.16.0] — 2026-04-30

### Added
- **LP Payout Engine**
  (`migrations/20260430000000_lp_payout_engine.sql`, `src/lp_payout/`).
- **Peg Integrity Monitor**
  (`migrations/20260430100000_peg_integrity_monitor.sql`).
- **Remittance Partners**
  (`migrations/20260429010000_remittance_partners.sql`).
- **Wallet Analytics** (`migrations/20260429000000_wallet_analytics.sql`).
- **SAR Workflow** (`migrations/20260428100001_sar_workflow.sql`).
- **KYB System** (`migrations/20260428100000_kyb_system.sql`).
- **Sanctions Screening Engine**
  (`migrations/20260428000001_sanctions_screening_engine.sql`, `src/sanctions/`).
- **Compliance Effectiveness Reports**
  (`migrations/20260428000000_compliance_effectiveness_reports.sql`,
  `src/compliance_effectiveness/`).
- **Performance SLA Management**
  (`migrations/20260427000000_performance_sla_management.sql`).
- **Async Webhook Delivery**
  (`migrations/20260424020000_async_webhook_delivery.sql`).
- **Merchant Webhooks**
  (`migrations/20260424015000_merchant_webhook_deliveries.sql`).
- **Merchant Loyalty Rewards**
  (`migrations/20260424010000_merchant_loyalty_rewards.sql`).
- **Merchants & Payment Intents**
  (`migrations/20260424000001_create_merchants_table.sql`,
  `migrations/20260424000002_merchant_payment_intents.sql`,
  `src/gateway/`).
- **HA Sharding Metadata**
  (`migrations/20260424000000_ha_sharding_metadata.sql`).
- **Multisig Treasury Controls**
  (`migrations/20260423100000_merchant_multisig_treasury_controls.sql`).
- **Proof-of-Reserves V1**
  (`migrations/20260423000002_proof_of_reserves_por.sql`).
- **Oracle Price Feed**
  (`migrations/20260423000001_oracle_price_feed.sql`, `src/oracle/`).
- **DEX Market Maker**
  (`migrations/20260423000000_dex_market_maker.sql`).

---

## [0.15.0] — 2026-04-05

### Added
- **Consumer Usage Analytics**
  (`migrations/20260405000000_consumer_usage_analytics.sql`,
  `src/analytics/`).
- **Security Compliance Framework**
  (`migrations/20260404000000_security_compliance_framework.sql`).
- **Mint Priority Scheduling**
  (`migrations/20260403000001_mint_priority_scheduling.sql`).
- **Adaptive Rate Limiting**
  (`migrations/20260403000000_adaptive_rate_limiting.sql`).
- **HMAC Signing Audit**
  (`migrations/20260402000000_hmac_signing_audit.sql`,
  `src/middleware/hmac_signing/`).
- **Data Classification Audit**
  (`migrations/20260402100000_data_classification_audit.sql`).
- **API Key Rotation & Expiry**
  (`migrations/20260401000000_api_key_rotation_expiry.sql`).

---

## [0.14.0] — 2026-03-30

### Added
- **AML / Financial Intelligence**
  (`migrations/20260330100000_aml_financial_intelligence.sql`, `src/aml/`).
- **Nostro Liquidity Management**
  (`migrations/20260330200000_nostro_liquidity_management.sql`).
- **Partner Reporting Engine**
  (`migrations/20260330300000_partner_reporting_engine.sql`).
- **Liquidity Pool Architecture**
  (`migrations/20260330000003_liquidity_pool_architecture.sql`).
- **Liquidity Monitor**
  (`migrations/20260330000002_liquidity_monitor_schema.sql`,
  `src/workers/liquidity_monitor_worker.rs`).
- **Emergency Intervention Schema**
  (`migrations/20260330000001_emergency_intervention_schema.sql`).
- **Bug Bounty Programme**
  (`migrations/20260330000000_bug_bounty_programme.sql`).

---

## [0.13.0] — 2026-03-29

### Added
- **Platform Key Management**
  (`migrations/20260329000000_platform_key_management.sql`,
  `src/key_management/`).
- **Transparency Portal / Reserve Vault**
  (`migrations/20260328300003_transparency_portal.sql`,
  `migrations/20260328300002_reserve_vault_schema.sql`).
- **Issuer Account Infrastructure**
  (`migrations/20260328300000_issuer_account_infrastructure.sql`).
- **Payload Encryption Keys**
  (`migrations/20260328200000_payload_encryption_keys.sql`,
  `src/crypto/`).
- **API Key Revocation Blacklist**
  (`migrations/20260328100000_api_key_revocation_blacklist.sql`).
- **cNGN Supply Monitoring**
  (`migrations/20260328010000_cngn_supply_monitoring.sql`,
  `src/workers/supply_monitor_worker.rs`).
- **Supply / Reserve Reconciliation**
  (`migrations/20260328020000_supply_reserve_reconciliation.sql`,
  `src/workers/reconciliation_worker.rs`).
- **Reconciliation Worker V1**
  (`migrations/20260328300001_reconciliation_worker.sql`).
- **DB Query Optimisation V2**
  (`migrations/20260328000000_db_query_optimisation_v2.sql`).

---

## [0.12.0] — 2026-03-27

### Added
- **mTLS Certificate Lifecycle**
  (`migrations/20260327120000_mtls_certificate_lifecycle.sql`).
- **Consumer Usage Analytics Schema**
  (`migrations/20260327150000_consumer_usage_analytics_schema.sql`).
- **Analytics Indexes**
  (`migrations/20260327000001_analytics_indexes.sql`).
- **API Key Generation**
  (`migrations/20260327000000_api_key_generation.sql`).
- **Geo Restriction** (`migrations/20260326113800_create_geo_restriction_schema.sql`).
- **IP Reputation** (`migrations/20260326110200_create_ip_reputation_schema.sql`).
- **Transaction History Indexes**
  (`migrations/20260326000002_transaction_history_indexes.sql`).

---

## [0.11.0] — 2026-03-26

### Added
- **Microservice-to-Microservice Authentication** (Issue #service-auth) —
  OAuth 2.0 Client Credentials flow, JWT service tokens (15-min TTL), proactive
  refresh at 20% lifetime, mTLS per-service certificates, service call
  allowlist with wildcard matching, impersonation prevention, Prometheus
  counters, audit log.
  (`migrations/20260326000001_service_identity.sql`, `src/service_auth/`).
- **IP Allowlist Management**
  (`migrations/20260326000000_ip_allowlist_management.sql`).

---

## [0.10.0] — 2026-03-25

### Added
- **Recurring Payment Schedules** (schema only) —
  `recurring_payment_schedules` + `recurring_payment_executions` tables;
  idempotency index on `(schedule_id, scheduled_at)`; worker query index.
  (`migrations/20260325000000_recurring_payments.sql`).
  *Implementation deferred — see v0.27.1.*
- **OAuth Clients**
  (`migrations/20260325000001_oauth_clients.sql`, `src/oauth/`).
- **Stellar Confirmation Worker**
  (`migrations/20260325000002_stellar_confirmation_worker.sql`,
  `src/workers/stellar_confirmation_worker.rs`).
- **Request Integrity Audit**
  (`migrations/20260325010000_request_integrity_audit.sql`,
  `src/middleware/request_integrity/`).
- **Exchange Rate History**
  (`migrations/20260325100000_exchange_rate_history.sql`,
  `src/rate_engine/`).

---

## [0.9.0] — 2026-03-24

### Added
- **Batch Transactions** (`migrations/20260324010000_batch_transactions.sql`,
  `src/batching/`).
- **Onramp Processor State Machine**
  (`migrations/20260324000002_onramp_processor_states.sql`,
  `src/workers/onramp_processor.rs`) — `pending → payment_received →
  processing → completed / refund_initiated → refunded`; idempotent state
  transitions via optimistic locking.
- **DB Maintenance Worker**
  (`migrations/20260324000001_db_maintenance_worker.sql`,
  `src/workers/maintenance.rs`).
- **API Key Scoping**
  (`migrations/20260324000000_api_key_scoping.sql`, `src/api_keys/`).

---

## [0.8.0] — 2026-02-21

### Added
- **Bill Processor Extensions**
  (`migrations/20260221000000_bill_processor_extensions.sql`,
  `src/workers/bill_processor.rs`).
- **Onramp Quotes** (`migrations/20260220100000_onramp_quotes.sql`).
- **Enhanced Fee Structures**
  (`migrations/20260220000000_enhanced_fee_structures.sql`).

---

## [0.7.0] — 2026-02-10

### Changed
- Rename `afri` → `cngn` throughout schema and codebase
  (`migrations/20260210000000_rename_afri_to_cngn.sql`).

### Added
- **Indexes & Constraints V2**
  (`migrations/20260209070000_indexes_constraints_performance.sql`).
- **AFRI / cNGN Operations & Rates Schema**
  (`migrations/20260209050000_afri_operations_and_rates_schema.sql`).
- **Core Indexes & Constraints**
  (`migrations/20260124000000_indexes_and_constraints.sql`).

---

## [0.6.0] — 2026-01-23

### Added
- **Payments Schema** (`migrations/20260123040000_implement_payments_schema.sql`,
  `src/payments/`) — `PaymentProvider` trait; Paystack, Flutterwave, M-Pesa
  providers; `PaymentProviderFactory` with country routing and fee comparison.
- **Core Schema** (`migrations/20260122120000_create_core_schema.sql`) —
  wallets, transactions, exchange rates, fee structures.

---

## [0.5.0] — 2025-01-01

### Added
- Notification history (`migrations/20250101120001_create_notification_history.up.sql`).

---

## [0.4.0] — 2024-11-01

### Added
- **Consumer Rate Limits**
  (`migrations/20241101000000_consumer_rate_limits.sql`).
- **SOR & Rebalancing**
  (`migrations/20241101000001_sor_and_rebalancing.sql`).

---

## [0.3.0] — 2024-03-26

### Added
- **KYC Tables** (`migrations/20240326000000_create_kyc_tables.sql`,
  `src/kyc/`, `src/verification/`).

---

## [0.2.0] — 2024-03-25

### Added
- **Refresh Tokens** (`migrations/20240325_create_refresh_tokens.sql`).
- **Consumers Bootstrap**
  (`migrations/20240325500000_create_consumers_bootstrap.sql`).

---

## [0.1.0] — 2024-03-24

### Added
- Initial repository scaffold — Rust / Axum backend for the Aframp platform.
- **OAuth Scopes** (`migrations/20240324_create_oauth_scopes.sql`).
- **Token Registry** (`migrations/20240324000001_create_token_registry.sql`).
- `Cargo.toml` with full dependency set (axum 0.8, sqlx 0.8, tokio 1.36,
  Stellar SDK, jemalloc/mimalloc allocators, OpenTelemetry).

---

[Unreleased]: https://github.com/kellymusk/Aframp-backend/compare/v0.27.1...HEAD
[0.27.1]: https://github.com/kellymusk/Aframp-backend/compare/v0.27.0...v0.27.1
[0.27.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.26.2...v0.27.0
[0.26.2]: https://github.com/kellymusk/Aframp-backend/compare/v0.26.1...v0.26.2
[0.26.1]: https://github.com/kellymusk/Aframp-backend/compare/v0.26.0...v0.26.1
[0.26.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.25.0...v0.26.0
[0.25.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.24.0...v0.25.0
[0.24.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.23.0...v0.24.0
[0.23.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.22.0...v0.23.0
[0.22.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.21.0...v0.22.0
[0.21.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.20.0...v0.21.0
[0.20.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.19.0...v0.20.0
[0.19.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.18.0...v0.19.0
[0.18.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/kellymusk/Aframp-backend/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/kellymusk/Aframp-backend/releases/tag/v0.1.0
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
