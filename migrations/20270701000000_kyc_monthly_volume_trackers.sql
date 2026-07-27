-- Monthly KYC volume tracking, kept separate from kyc_volume_trackers (which is
-- keyed per calendar day and therefore cannot represent a running monthly total:
-- a new day gets a new row via ON CONFLICT (consumer_id, date), so the old
-- per-row `monthly_volume` column silently resets every day instead of every month.
--
-- This table holds a single row per consumer. `month_start` records which
-- calendar month the row's `monthly_volume` belongs to, so a stale row (from a
-- prior month) can be detected and rolled over atomically -- either lazily, in
-- the same UPSERT statement that records a transaction, or via an explicit
-- reset job. See KycRepository::upsert_monthly_volume / reset_stale_monthly_counters.
CREATE TABLE kyc_monthly_volume_trackers (
    consumer_id UUID PRIMARY KEY REFERENCES consumers(id) ON DELETE CASCADE,
    month_start DATE NOT NULL,
    monthly_volume DECIMAL(20,8) NOT NULL DEFAULT 0,
    last_reset_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_kyc_monthly_volume_trackers_month_start ON kyc_monthly_volume_trackers(month_start);
