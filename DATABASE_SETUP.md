# Database Setup Guide

This guide explains how to set up the PostgreSQL database for the Aframp backend.

## Prerequisites

- PostgreSQL 14+ installed and running
- `sqlx` CLI tool installed

## Database Creation

1. **Create the database**:
   ```bash
   sudo -u postgres createdb aframp
   sudo -u postgres createuser -s $USER
   ```

2. **Install sqlx CLI** (if not already installed):
   ```bash
   cargo install --features postgres sqlx-cli
   ```

3. **Run migrations**:
   ```bash
   DATABASE_URL=postgresql://localhost/aframp sqlx migrate run
   ```

## Database Schema Overview

The database contains the following tables:

### Core Tables

1. **users** - User accounts for non-custodial operations
   - `id` (UUID) - Primary key
   - `email` (TEXT) - Unique email address
   - `phone` (TEXT) - Optional phone number
   - `created_at`, `updated_at` (TIMESTAMPTZ)

2. **wallets** - Connected wallet addresses
   - `id` (UUID) - Primary key
   - `user_id` (UUID) - Foreign key to users
   - `wallet_address` (VARCHAR) - Unique blockchain address
   - `chain` (TEXT) - Blockchain network (stellar, ethereum, bitcoin)
   - `has_cngn_trustline` (BOOLEAN) - Whether CNGN trustline exists
   - `cngn_balance` (NUMERIC) - Cached CNGN balance
   - `last_balance_check` (TIMESTAMPTZ) - Last balance refresh timestamp
   - `created_at`, `updated_at` (TIMESTAMPTZ)

3. **transactions** - All payment operations
   - `transaction_id` (UUID) - Primary key
   - `wallet_address` (VARCHAR) - Foreign key to wallets
   - `type` (TEXT) - Operation type (onramp, offramp, bill_payment)
   - `from_currency`, `to_currency` (TEXT) - Currency codes
   - `from_amount`, `to_amount`, `afri_amount` (NUMERIC) - Transaction amounts
   - `status` (TEXT) - Transaction status
   - `payment_provider` (TEXT) - Payment provider used
   - `payment_reference` (TEXT) - Provider reference
   - `blockchain_tx_hash` (TEXT) - On-chain transaction hash
   - `error_message` (TEXT) - Error details if failed
   - `metadata` (JSONB) - Provider-specific data
   - `created_at`, `updated_at` (TIMESTAMPTZ)

4. **cngn_trustlines** - CNGN trustline establishment
   - `id` (UUID) - Primary key
   - `wallet_address` (VARCHAR) - Unique wallet address
   - `established_at` (TIMESTAMPTZ) - When trustline was established
   - `metadata` (JSONB) - Chain-specific metadata
   - `created_at`, `updated_at` (TIMESTAMPTZ)

### Lookup Tables

5. **transaction_statuses** - Extensible transaction status codes
   - `code` (TEXT) - Status code (pending, processing, completed, failed)
   - `description` (TEXT) - Human-readable description
   - `created_at`, `updated_at` (TIMESTAMPTZ)

## Migration Management

### Running Migrations

```bash
# Run all pending migrations
DATABASE_URL=postgresql://localhost/aframp sqlx migrate run

# Check migration status
DATABASE_URL=postgresql://localhost/aframp sqlx migrate info

# Revert last migration
DATABASE_URL=postgresql://localhost/aframp sqlx migrate revert
```

### Adding New Migrations

```bash
# Create a new migration
sqlx migrate add migration_name

# This creates files:
# - migrations/YYYYMMDDHHMMSS_migration_name.sql (up migration)
# - migrations/YYYYMMDDHHMMSS_migration_name.sql (down migration)
```

## Database Connection

The application uses the following environment variables for database configuration:

```bash
DATABASE_URL=postgresql://user:password@localhost:5432/aframp
DATABASE_MAX_CONNECTIONS=20
```

## Troubleshooting

### Common Issues

1. **Connection refused**:
   ```bash
   # Check if PostgreSQL is running
   sudo systemctl status postgresql
   
   # Start PostgreSQL if needed
   sudo systemctl start postgresql
   ```

2. **Database does not exist**:
   ```bash
   # Create the database
   sudo -u postgres createdb aframp
   ```

3. **Permission denied**:
   ```bash
   # Create user with superuser privileges
   sudo -u postgres createuser -s $USER
   ```

4. **Migration errors**:
   ```bash
   # Check current migration status
   DATABASE_URL=postgresql://localhost/aframp sqlx migrate info
   
   # Revert and re-run if needed
   DATABASE_URL=postgresql://localhost/aframp sqlx migrate revert
   DATABASE_URL=postgresql://localhost/aframp sqlx migrate run
   ```

### Useful Queries

```sql
-- Check database connection
SELECT version();

-- List all tables
SELECT table_name FROM information_schema.tables WHERE table_schema = 'public';

-- Check migration status
SELECT * FROM _sqlx_migrations;

-- Count records in key tables
SELECT COUNT(*) FROM users;
SELECT COUNT(*) FROM wallets;
SELECT COUNT(*) FROM transactions;
```

## Backup and Restore

### Backup
```bash
pg_dump -h localhost -U username -d aframp > backup.sql
```

### Restore
```bash
psql -h localhost -U username -d aframp < backup.sql
```

## Performance Considerations

- Indexes are created on frequently queried columns
- Use `NUMERIC` for monetary values to avoid precision issues
- `updated_at` columns are automatically maintained via triggers
- JSONB is used for flexible metadata storage