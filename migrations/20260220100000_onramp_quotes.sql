-- migrate:up
-- Onramp quotes: time-bound NGN → cNGN conversion quotes

CREATE TABLE IF NOT EXISTS onramp_quotes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    quote_id UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    amount_ngn NUMERIC(36, 18) NOT NULL CHECK (amount_ngn > 0),
    exchange_rate NUMERIC(36, 18) NOT NULL CHECK (exchange_rate > 0),
    gross_cngn NUMERIC(36, 18) NOT NULL CHECK (gross_cngn >= 0),
    fee_cngn NUMERIC(36, 18) NOT NULL DEFAULT 0 CHECK (fee_cngn >= 0),
    net_cngn NUMERIC(36, 18) NOT NULL CHECK (net_cngn >= 0),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'consumed', 'expired')),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE onramp_quotes IS 'Time-bound NGN to cNGN conversion quotes for onramp flow.';
COMMENT ON COLUMN onramp_quotes.status IS 'pending: active quote; consumed: used by initiate; expired: past TTL.';

CREATE INDEX IF NOT EXISTS idx_onramp_quotes_quote_id ON onramp_quotes(quote_id);
CREATE INDEX IF NOT EXISTS idx_onramp_quotes_status ON onramp_quotes(status);
CREATE INDEX IF NOT EXISTS idx_onramp_quotes_expires_at ON onramp_quotes(expires_at);

-- migrate:down
DROP TABLE IF EXISTS onramp_quotes;
