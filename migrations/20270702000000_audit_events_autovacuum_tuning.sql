-- Tune autovacuum for high-write tables so dead tuples are reclaimed before
-- they cause table bloat. Default autovacuum_vacuum_scale_factor (20% of the
-- table) is far too high for a table that accumulates dead tuples as
-- rapidly as audit_events; lower the scale factor/threshold and raise the
-- cost limit so autovacuum runs more often and completes faster.

ALTER TABLE audit_events SET (
    autovacuum_vacuum_scale_factor = 0.02,
    autovacuum_vacuum_threshold = 1000,
    autovacuum_analyze_scale_factor = 0.02,
    autovacuum_analyze_threshold = 1000,
    autovacuum_vacuum_cost_limit = 2000
);
