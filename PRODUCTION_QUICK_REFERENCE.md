# Production Quick Reference

## Setup

```bash
# Initial setup
./setup-production-db.sh

# Build release
cargo build --release

# Install service
sudo cp aframp-backend.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable aframp-backend
sudo systemctl start aframp-backend
```

## Service Management

```bash
# Start service
sudo systemctl start aframp-backend

# Stop service
sudo systemctl stop aframp-backend

# Restart service
sudo systemctl restart aframp-backend

# Check status
sudo systemctl status aframp-backend

# View logs
sudo journalctl -u aframp-backend -f

# View errors only
sudo journalctl -u aframp-backend -p err -n 50
```

## Database Operations

```bash
# Connect to database
psql -U aframp_user -d aframp

# Backup database
./backup-db.sh

# Restore database
gunzip -c /var/backups/postgresql/aframp/backup_file.sql.gz | psql -U aframp_user -d aframp

# Monitor database
./monitor-db.sh

# Check database size
psql -U aframp_user -d aframp -c "SELECT pg_size_pretty(pg_database_size('aframp'));"

# Vacuum database
psql -U aframp_user -d aframp -c "VACUUM ANALYZE;"
```

## Monitoring

```bash
# Application health
curl http://localhost:8000/health

# Database connections
psql -U aframp_user -d aframp -c "SELECT count(*) FROM pg_stat_activity;"

# Slow queries
psql -U aframp_user -d aframp -c "
SELECT query, mean_time FROM pg_stat_statements 
ORDER BY mean_time DESC LIMIT 10;"

# Disk usage
df -h

# Memory usage
free -h

# CPU usage
top
```

## Deployment

```bash
# Pull latest code
git pull origin main

# Build release
cargo build --release

# Restart service
sudo systemctl restart aframp-backend

# Verify deployment
curl http://localhost:8000/health
sudo journalctl -u aframp-backend -n 50
```

## Troubleshooting

```bash
# Service won't start
sudo journalctl -u aframp-backend -n 100
sudo systemctl status aframp-backend

# Database connection issues
psql -U aframp_user -d aframp -c "SELECT 1;"
sudo systemctl status postgresql

# High memory usage
ps aux | grep Aframp-Backend
ps aux | grep postgres

# Check configuration
cat .env.production
```

## Emergency Procedures

```bash
# Stop everything
sudo systemctl stop aframp-backend
sudo systemctl stop redis
sudo systemctl stop postgresql

# Restore from backup
gunzip -c /var/backups/postgresql/aframp/latest_backup.sql.gz | \
  psql -U aframp_user -d aframp

# Start everything
sudo systemctl start postgresql
sudo systemctl start redis
sudo systemctl start aframp-backend
```

## Performance Tuning

```bash
# PostgreSQL config
sudo -u postgres psql -c "SHOW shared_buffers;"
sudo -u postgres psql -c "SHOW max_connections;"

# Reload PostgreSQL config
sudo systemctl reload postgresql

# Clear query cache
psql -U aframp_user -d aframp -c "SELECT pg_stat_statements_reset();"
```

## Security

```bash
# Check open ports
sudo netstat -tulpn

# Check firewall
sudo ufw status

# Update system
sudo apt update && sudo apt upgrade

# Check failed login attempts
sudo journalctl _SYSTEMD_UNIT=sshd.service | grep "Failed password"
```

## Useful SQL Queries

```sql
-- Table sizes
SELECT 
    tablename,
    pg_size_pretty(pg_total_relation_size(tablename::text)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(tablename::text) DESC;

-- Active queries
SELECT pid, usename, state, query 
FROM pg_stat_activity 
WHERE state != 'idle';

-- Kill long-running query
SELECT pg_terminate_backend(pid) 
FROM pg_stat_activity 
WHERE pid = <pid_number>;

-- Index usage
SELECT 
    schemaname,
    tablename,
    indexname,
    idx_scan,
    idx_tup_read,
    idx_tup_fetch
FROM pg_stat_user_indexes
ORDER BY idx_scan DESC;
```

## Environment Files

```bash
# Production
.env.production

# Test
.env.test

# Development
.env
```

## Important Paths

```bash
# Application
/path/to/aframp-backend/

# Binary
target/release/Aframp-Backend

# Logs
sudo journalctl -u aframp-backend

# Database backups
/var/backups/postgresql/aframp/

# PostgreSQL logs
/var/log/postgresql/

# Service file
/etc/systemd/system/aframp-backend.service
```

## Common Issues

### Issue: Service fails to start
```bash
# Check logs
sudo journalctl -u aframp-backend -n 100

# Check environment
cat .env.production

# Check binary
ls -la target/release/Aframp-Backend

# Test manually
./target/release/Aframp-Backend
```

### Issue: Database connection refused
```bash
# Check PostgreSQL
sudo systemctl status postgresql

# Check credentials
psql -U aframp_user -d aframp

# Check pg_hba.conf
sudo cat /etc/postgresql/*/main/pg_hba.conf
```

### Issue: Out of memory
```bash
# Check memory
free -h

# Check processes
ps aux --sort=-%mem | head

# Restart service
sudo systemctl restart aframp-backend
```

### Issue: Slow performance
```bash
# Check slow queries
./monitor-db.sh

# Vacuum database
psql -U aframp_user -d aframp -c "VACUUM ANALYZE;"

# Check indexes
psql -U aframp_user -d aframp -c "
SELECT * FROM pg_stat_user_indexes WHERE idx_scan = 0;"
```

## Maintenance Schedule

**Daily:**
- Check logs: `sudo journalctl -u aframp-backend -p err`
- Verify backups: `ls -lh /var/backups/postgresql/aframp/`

**Weekly:**
- Run monitoring: `./monitor-db.sh`
- Check disk space: `df -h`

**Monthly:**
- Update system: `sudo apt update && sudo apt upgrade`
- Review performance: Check slow queries
- Test backup restore

## Contact Information

- **Documentation**: See PRODUCTION_DEPLOYMENT.md
- **Setup Script**: ./setup-production-db.sh
- **Monitoring**: ./monitor-db.sh
- **Backup**: ./backup-db.sh
