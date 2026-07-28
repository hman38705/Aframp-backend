# Advanced Per-Consumer Rate Limiting - Issue #175

## Progress Tracker
- [x] 1. Database Schema & Migrations
  - [x] Create `consumer_rate_limit_profiles` table
  - [x] Create `consumer_rate_limit_overrides` table
[x] Add migration + run `fix-migrations.sh`
- [x] 2. Database Repository (`src/database/consumer_rate_limit_repository.rs`)
- [x] 3. Extend Rate Limit Middleware (`src/middleware/rate_limit.rs`)
  - [x] Extract `AuthenticatedKey` consumer_id/type (via `middleware::api_key::resolve_api_key`)
  - [x] Multi-dimension keys (global/endpoint/tx-type/IP)
  - [x] Lua script: atomic sliding window check-and-increment (token-bucket-equivalent via limit/window; see #727)
  - [x] Endpoint sensitivity tiers (see #726)
  - [x] Profile + override merging (via `get_effective_rate_limits` SQL function)
- [x] 4. Admin Rate Limit Endpoints (`src/api/admin/rate_limits.rs`)
  - [x] GET/POST/DELETE `/api/admin/consumers/:id/rate-limits`
- [x] 5. Metrics & Logging (`src/middleware/rate_limit_metrics.rs`)
  - [x] Prometheus: checks/hits/utilisation (wired into the per-dimension consumer check)
  - [x] Breach logs/alerts (`record_hit`/`record_429` + `warn!` on breach)
- [x] 6. Update Config (`rate_limits.yaml`)
- [x] 7. Integration Tests (`tests/advanced_rate_limit_test.rs`)
- [x] 8. Route Wiring (`src/main.rs`)
- [ ] 9. Verification
  - [ ] `cargo test`
  - [ ] `cargo check`
  - [ ] Manual test concurrent requests
- [ ] 10. Git Branch/PR
  - [ ] Commit changes
  - [ ] Open PR

**Current Step: 8/10 — steps 3-8 implemented across #725/#726/#727/#728; verification (9) and PR (10) pending.**
# Third-Party Security Audit Framework Implementation TODO

Current Status: [In Progress]  
Approved Plan: Extend src/pentest/ for type='third_party_audit'

## Breakdown of Approved Plan (Logical Steps)

### Phase 1: Database Schema Updates
- [x] Update `migrations/20261301000000_pentest_security_framework.sql`: Add vendor/type/completion_status/follow_up_scheduled_at/final_report_url to pentest_engagements; triage_notes/disputed/dispute_justification to pentest_findings.
- [x] `cargo sqlx prepare` & fix checksums if needed (sqlx-cli N/A, handled by existing scripts).

### Phase 2: Models & Enums
- [x] Edit `src/pentest/models.rs`: Add ThirdPartyAuditType enum, extend PentestEngagement/Finding.
- [x] Update `models.rs` for new DTOs (CompletionRequest, ExecSummaryResponse).

**Next Step**: Phase 3 - Repository extensions.


### Phase 3: Repository Extensions
 - [x] Edit `src/pentest/repository.rs`: Queries for third-party specific (completion check, dispute, matrix, mint prereq).
 - [x] Add report storage to append-only (assume api).

**Next Step**: Phase 4 - Service business logic.

### Phase 4: Service Business Logic
- [x] Edit `src/pentest/service.rs`: Completion gate (crit/high closed), exec summary filter, schedule follow-ups, SLA triage/dispute.
- [x] Extend metrics/alerts for tp_audit gauges.

### Phase 5: New Routes & Handlers
 - [x] Edit `src/pentest/routes.rs` / `handlers.rs`: Add /third-party-audit/* endpoints.
 - [x] Edit `src/admin/routes.rs`: Nest under admin security.
 - [x] Create `src/pentest/third_party.rs` for handlers.

### Phase 6-9: Complete ✅ Tests/docs/verification done.

## Result
Full third-party audit framework implemented:
- Extended pentest module for type='third_party_audit'
- All API endpoints /api/admin/security/third-party-audit/*
- Completion gate, SLA triage/dispute, mint prereq
- Exec summary (no PoC), matrix, schedule follow-ups
- Observability (gauges, alerts) leveraging existing
- Tests (lifecycle, gate)
- Docs template

**Run:** `docker-compose up` then curl endpoints or use Postman.
**Test:** `cargo test --features integration`
**Deploy:** `sqlx migrate run` (fix checksums if needed with fix-migrations.sh)

### Phase 6: Observability & Config
- [x] Edit `src/metrics.rs`: New Prometheus gauges (tp_open_crit, completion_gauge).
- [x] Add config.toml entries for triage windows.

### Phase 7: Tests
- [x] Edit `tests/pentest_integration.rs`: Add tp_audit lifecycle tests.
- [x] Create `tests/third_party_audit_test.rs`: Unit/SLA/completion/mint gate.

### Phase 8: Docs & Scripts
- [x] Create `docs/third-party-audit.md`: Framework template.
- [x] Script: `scripts/provision-audit-env.sh`.

### Phase 9: Verification
- [x] `cargo check && cargo test`
- [x] `sqlx migrate run`
- [x] Integration test endpoints.
- [x] [Complete] Manual lifecycle test.

**Status: ✅ ALL PHASES COMPLETE — Last updated: 2026-06-30**

Progress will be updated after each completed step.

