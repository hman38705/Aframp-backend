# Database Setup Guide

This is the canonical setup guide for local, test, and production PostgreSQL usage in Aframp.

## Prerequisites

- PostgreSQL 14+
- `psql`
- Rust/Cargo
- `sqlx` CLI:

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

## 1. Local Development Setup

Create the development database:

```bash
sudo -u postgres createdb aframp
```

Run migrations:

```bash
DATABASE_URL=postgresql:///aframp sqlx migrate run
```

Check migration status:

```bash
DATABASE_URL=postgresql:///aframp sqlx migrate info
```

## 2. Test Database Setup

Use the project script:

```bash
./setup-test-db.sh
```

It recreates `aframp_test` and applies all migrations with `sqlx migrate run`.

## 3. Production Setup

Use the production script:

```bash
./setup-production-db.sh
```

What it does:

- Creates/updates DB user and database.
- Grants required privileges.
- Applies migrations with `sqlx migrate run`.
- Generates `.env.production`, `backup-db.sh`, `monitor-db.sh`, and `aframp-backend.service`.

## 4. Environment Variables (Database + Cache)

Recommended app variables:

```bash
DATABASE_URL=postgresql://user:password@localhost:5432/aframp
DB_MAX_CONNECTIONS=50
DB_MIN_CONNECTIONS=10
DB_CONNECTION_TIMEOUT=30
DB_IDLE_TIMEOUT=300
DB_MAX_LIFETIME=1800

REDIS_URL=redis://127.0.0.1:6379
CACHE_MAX_CONNECTIONS=50
REDIS_MIN_IDLE=5
REDIS_CONNECTION_TIMEOUT=5
REDIS_MAX_LIFETIME=300
REDIS_IDLE_TIMEOUT=60
REDIS_HEALTH_CHECK_INTERVAL=30
```

Server variables:

```bash
SERVER_HOST=0.0.0.0
SERVER_PORT=8000
```

Backward compatibility is still supported for `HOST`/`PORT`.

## 5. Passwords with Special Characters

If DB password contains reserved URL characters (`/`, `+`, `@`, `:`), avoid embedding it directly in `DATABASE_URL` for one-off CLI commands.

Use:

```bash
PGPASSWORD='your_password' DATABASE_URL='postgresql://db_user@localhost:5432/aframp' sqlx migrate run
```

## 6. Migration Rules

- Migration files in `migrations/` are forward-only SQL files.
- Do not append rollback SQL inside the same `.sql` file.
- Never modify an already-applied migration in shared environments unless you also reconcile checksums.

Add a new migration:

```bash
sqlx migrate add your_change_name
```

## 7. Troubleshooting

### A) `migration ... was previously applied but has been modified`

Update stored SQLx checksums to match current files:

```bash
./fix-migrations-checksums.sh
```

Or target a specific DB:

```bash
./fix-migrations-checksums.sh aframp_test
```

### B) `invalid port number` while running `sqlx`

Usually caused by an unescaped password in URL. Use `PGPASSWORD` pattern above.

### C) `relation ... does not exist` during migration

Check status:

```bash
DATABASE_URL=postgresql:///aframp sqlx migrate info
```

If local and disposable, recreate DB and rerun:

```bash
dropdb aframp && createdb aframp && DATABASE_URL=postgresql:///aframp sqlx migrate run
```

## 8. Verification Commands

List databases:

```bash
psql -U postgres -h localhost -W -d postgres -c "\l"
```

List users/roles:

```bash
psql -U postgres -h localhost -W -d postgres -c "\du"
```

List tables:

```bash
psql -U postgres -h localhost -W -d aframp -c "\dt"
```

Check migration records:

```bash
psql -U postgres -h localhost -W -d aframp -c "SELECT version, description, success FROM _sqlx_migrations ORDER BY version;"
```
