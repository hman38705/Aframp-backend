# Test Environment Setup

## Summary

I've created a separate test database environment for your Aframp backend project.

## What Was Done

### 1. Fixed Migration Issues
- Renamed `migrations/004_indexes_and_constraints.sql` to `migrations/20260124000000_indexes_and_constraints.sql` for proper ordering
- Fixed column name references (`transaction_type` → `type`, `amount` → `from_amount`, etc.)
- Removed references to non-existent tables (`trustline_operations`, `exchange_rates`)
- Fixed table references (`trustline_operations` → `afri_trustlines`)

### 2. Created Test Database Setup Script
- **File**: `setup-test-db.sh`
- Creates a separate `aframp_test` database
- Runs migrations without the problematic `migrate:down` sections
- Usage: `./setup-test-db.sh`

### 3. Created Test Environment Configuration
- **File**: `.env.test`
- Configured for test database: `postgresql:///aframp_test`
- Uses port 8001 (different from production port 8000)
- Debug logging enabled

### 4. Created Test Server Runner
- **File**: `run-test-server.sh`
- Loads test environment variables
- Runs backend with test database
- Usage: `./run-test-server.sh`

### 5. Added Request Logging Middleware
- Updated `src/main.rs` to include logging middleware
- Added middleware module to `src/lib.rs`
- Every HTTP request now logs:
  - Request ID (UUID)
  - Method and path
  - Response status code
  - Duration in milliseconds
  - Slow requests (>200ms) logged as warnings

## How to Use

### Setup Test Database
```bash
./setup-test-db.sh
```

### Run Backend with Test Database
```bash
./run-test-server.sh
```

### Run Backend with Production Database
```bash
cargo run
```

### Connect to Test Database
```bash
psql -d aframp_test
```

### View Tables in Test Database
```bash
psql -d aframp_test -c "\dt"
```

## Test Database Tables

The test database includes:
- `users` - User accounts
- `wallets` - Wallet addresses
- `transactions` - Payment transactions
- `afri_trustlines` - AFRI trustline records
- `transaction_statuses` - Transaction status lookup
- `payment_provider_configs` - Payment provider settings
- `payment_methods` - User payment methods
- `bill_payments` - Bill payment records
- `webhook_events` - Webhook event log
- `webhook_deliveries` - Webhook delivery tracking

## Request Logging

With the logging middleware enabled, every request will log:

```
INFO Request started request_id=<uuid> method=GET path=/health
INFO Request completed request_id=<uuid> method=GET path=/health status=200 duration_ms=5
```

Slow requests (>200ms) will be logged as warnings:
```
WARN Slow request completed request_id=<uuid> method=GET path=/api/... status=200 duration_ms=350
```

## Environment Variables

### Production (.env)
- `DATABASE_URL=postgresql:///aframp`
- `PORT=8000`
- `RUST_LOG=info`

### Test (.env.test)
- `DATABASE_URL=postgresql:///aframp_test`
- `PORT=8001`
- `RUST_LOG=debug`

## Next Steps

1. **Fix setup.sh**: The original `setup.sh` script has migration issues. Consider using the new `setup-test-db.sh` approach
2. **Test the logging**: Make requests to your backend and verify logs appear
3. **Add more endpoints**: The logging middleware will automatically log all new endpoints you add

## Troubleshooting

### Backend not starting
- Check if PostgreSQL is running: `pg_isready`
- Check if Redis is running: `redis-cli ping`
- Check database connection: `psql -d aframp_test -c "SELECT 1;"`

### Migrations failing
- Use the test setup script instead of sqlx migrate
- The sqlx migrations have issues with the `migrate:down` sections

### Request logs not showing
- Ensure `RUST_LOG=info` or `RUST_LOG=debug` is set
- Check that the middleware is properly configured in `src/main.rs`
