# Disaster Recovery Test Log

This log records every automated DR backup-restore drill executed by
[`.github/workflows/dr_backup_restore_verify.yml`](../.github/workflows/dr_backup_restore_verify.yml).
Each run restores the latest immutable backup to an ephemeral Postgres
instance, verifies data integrity (row counts + content hash on critical
tables), measures RTO, and appends a row below.

- **RTO policy ceiling:** 4 hours (14,400s) — per BIA (`dr_bia_entries`), see `migrations/20270428000001_dr_bcp_schema.sql`
- **Operational RTO target:** 900s (15 min) for critical services
- **Cadence:** daily automated check + monthly full drill (`workflow_dispatch` / monthly cron)

Full run history with RPO/RTO and backup metadata is also persisted in
the `dr_restore_test_runs` table via the DR/BCP API — this file is the
human-readable summary.

| Date (UTC) | Trigger | Result | Restore Duration | RTO Target Met (≤4h) | Row Count Check | Hash Check | Run |
|---|---|---|---|---|---|---|---|
<!-- New rows are appended above this line by the dr_backup_restore_verify workflow. Do not edit existing rows. -->
