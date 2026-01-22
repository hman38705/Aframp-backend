# Database Migrations

This directory contains SQL migration files for the Aframp backend database schema.

## Migration Files

- `004_indexes_and_constraints.sql` - Database indexes, constraints, and performance optimizations

## Running Migrations

```bash
# Run all pending migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert

# Check migration status
sqlx migrate info
```

## Migration Structure

Each migration file includes:
- **UP Migration**: Schema changes to apply
- **DOWN Migration**: Rollback instructions
- **Comments**: Explaining purpose and design decisions
- **Performance Notes**: Expected query performance targets

## Database Setup

Ensure your `.env` file has the `DATABASE_URL` configured:

```env
DATABASE_URL=postgresql://user:password@localhost/aframp
```

## Performance Monitoring

After migrations, monitor index usage:

```sql
-- Check index usage statistics
SELECT * FROM pg_stat_user_indexes WHERE schemaname = 'public';

-- Find unused indexes
SELECT * FROM pg_stat_user_indexes WHERE idx_scan = 0;
```

## Testing Migrations

1. Test on fresh database: `sqlx migrate run`
2. Test rollback: `sqlx migrate revert`
3. Test with production-like data volumes
4. Verify query performance with `EXPLAIN ANALYZE`
