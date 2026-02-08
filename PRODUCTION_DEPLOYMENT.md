# Production Deployment Guide

## Overview

This guide covers deploying the Aframp backend to a production environment with proper security, monitoring, and backup strategies.

## Prerequisites

- Ubuntu/Debian Linux server
- PostgreSQL 14+
- Redis 6+
- Rust toolchain
- Sudo access
- Domain name (optional, for HTTPS)

## Quick Start

```bash
# Run the production setup script
./setup-production-db.sh
```

This script will:
- Create a production database with a secure user
- Generate a strong password
- Apply production optimizations
- Set up backup scripts
- Create monitoring tools
- Generate systemd service file

## Manual Setup Steps

### 1. Database Setup

```bash
# Create production database
sudo -u postgres createdb aframp

# Create dedicated user with strong password
sudo -u postgres psql -c "CREATE USER aframp_user WITH ENCRYPTED PASSWORD 'your_secure_password';"

# Grant privileges
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE aframp TO aframp_user;"
```

### 2. Run Migrations

```bash
# Apply core schema
sed '/-- migrate:down/,$d' migrations/20260122120000_create_core_schema.sql | \
  psql -U aframp_user -d aframp -v ON_ERROR_STOP=1

# Apply payment schema
sed '/-- migrate:down/,$d' migrations/20260123040000_implement_payments_schema.sql | \
  psql -U aframp_user -d aframp -v ON_ERROR_STOP=1
```

### 3. Configure Environment

Create `.env.production`:

```bash
DATABASE_URL=postgresql://aframp_user:your_password@localhost/aframp
REDIS_URL=redis://127.0.0.1:6379
RUST_LOG=info
HOST=0.0.0.0
PORT=8000
STELLAR_NETWORK=mainnet
STELLAR_REQUEST_TIMEOUT=30
STELLAR_MAX_RETRIES=3
STELLAR_HEALTH_CHECK_INTERVAL=60

# Payment Provider Keys
PAYSTACK_SECRET_KEY=sk_live_your_key_here
PAYSTACK_BASE_URL=https://api.paystack.co

# Security
JWT_SECRET=your_jwt_secret_here
```

### 4. Build Release Binary

```bash
# Build optimized release binary
cargo build --release

# Binary location: target/release/Aframp-Backend
```

### 5. Set Up Systemd Service

```bash
# Copy service file
sudo cp aframp-backend.service /etc/systemd/system/

# Reload systemd
sudo systemctl daemon-reload

# Enable service to start on boot
sudo systemctl enable aframp-backend

# Start the service
sudo systemctl start aframp-backend

# Check status
sudo systemctl status aframp-backend
```

## Production Optimizations

### PostgreSQL Configuration

The setup script applies these optimizations:

```sql
-- Connection pooling
max_connections = 200
shared_buffers = 256MB
effective_cache_size = 1GB

-- Performance
work_mem = 4MB
maintenance_work_mem = 64MB
checkpoint_completion_target = 0.9

-- WAL settings
wal_buffers = 16MB
min_wal_size = 1GB
max_wal_size = 4GB

-- Query optimization
default_statistics_target = 100
random_page_cost = 1.1
effective_io_concurrency = 200
```

### Application Configuration

```toml
# Cargo.toml - Release profile
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

## Security Best Practices

### 1. Database Security

```bash
# Restrict PostgreSQL to local connections only
# Edit /etc/postgresql/*/main/pg_hba.conf
local   aframp    aframp_user    md5
host    aframp    aframp_user    127.0.0.1/32    md5

# Reload PostgreSQL
sudo systemctl reload postgresql
```

### 2. Firewall Configuration

```bash
# Allow only necessary ports
sudo ufw allow 22/tcp    # SSH
sudo ufw allow 80/tcp    # HTTP
sudo ufw allow 443/tcp   # HTTPS
sudo ufw enable
```

### 3. Environment Variables

```bash
# Never commit .env.production to git
echo ".env.production" >> .gitignore

# Set proper permissions
chmod 600 .env.production
```

### 4. SSL/TLS Setup

Use a reverse proxy like Nginx with Let's Encrypt:

```nginx
server {
    listen 443 ssl http2;
    server_name api.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/api.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/api.yourdomain.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## Backup Strategy

### Automated Backups

```bash
# Add to crontab for daily backups at 2 AM
crontab -e

# Add this line:
0 2 * * * /path/to/aframp-backend/backup-db.sh
```

### Manual Backup

```bash
# Create backup
./backup-db.sh

# Restore from backup
gunzip -c /var/backups/postgresql/aframp/aframp_backup_TIMESTAMP.sql.gz | \
  psql -U aframp_user -d aframp
```

### Backup to Remote Storage

```bash
# Install AWS CLI or similar
sudo apt install awscli

# Modify backup-db.sh to upload to S3
aws s3 cp $BACKUP_FILE s3://your-bucket/backups/
```

## Monitoring

### Database Monitoring

```bash
# Run monitoring script
./monitor-db.sh

# Check database size
psql -U aframp_user -d aframp -c "
SELECT pg_size_pretty(pg_database_size('aframp'));
"

# Check active connections
psql -U aframp_user -d aframp -c "
SELECT count(*) FROM pg_stat_activity;
"
```

### Application Monitoring

```bash
# View logs
sudo journalctl -u aframp-backend -f

# Check service status
sudo systemctl status aframp-backend

# View recent errors
sudo journalctl -u aframp-backend -p err -n 50
```

### Performance Monitoring

```bash
# Install monitoring tools
sudo apt install postgresql-contrib

# Enable pg_stat_statements
psql -U aframp_user -d aframp -c "CREATE EXTENSION pg_stat_statements;"

# View slow queries
psql -U aframp_user -d aframp -c "
SELECT query, calls, total_time, mean_time 
FROM pg_stat_statements 
ORDER BY mean_time DESC 
LIMIT 10;
"
```

## Scaling Considerations

### Vertical Scaling

1. **Increase server resources**
   - More RAM for PostgreSQL caching
   - More CPU cores for concurrent requests
   - SSD storage for faster I/O

2. **Optimize PostgreSQL**
   ```sql
   -- Adjust based on available RAM
   shared_buffers = '25% of RAM'
   effective_cache_size = '75% of RAM'
   ```

### Horizontal Scaling

1. **Database Read Replicas**
   - Set up PostgreSQL streaming replication
   - Route read queries to replicas
   - Keep writes on primary

2. **Load Balancing**
   - Use Nginx or HAProxy
   - Multiple backend instances
   - Session affinity if needed

3. **Caching Layer**
   - Redis for frequently accessed data
   - Cache exchange rates
   - Cache user sessions

## Troubleshooting

### Database Connection Issues

```bash
# Check PostgreSQL is running
sudo systemctl status postgresql

# Check connections
psql -U aframp_user -d aframp -c "SELECT count(*) FROM pg_stat_activity;"

# Check logs
sudo tail -f /var/log/postgresql/postgresql-*-main.log
```

### High Memory Usage

```bash
# Check PostgreSQL memory
ps aux | grep postgres

# Adjust shared_buffers if needed
sudo -u postgres psql -c "ALTER SYSTEM SET shared_buffers = '128MB';"
sudo systemctl restart postgresql
```

### Slow Queries

```bash
# Enable query logging
sudo -u postgres psql -c "ALTER SYSTEM SET log_min_duration_statement = 1000;"
sudo systemctl reload postgresql

# Analyze slow queries
./monitor-db.sh
```

## Maintenance

### Regular Tasks

**Daily:**
- Check application logs
- Monitor disk space
- Verify backups completed

**Weekly:**
- Review slow query log
- Check database size growth
- Update dependencies

**Monthly:**
- Security updates
- Performance review
- Backup restoration test

### Database Maintenance

```bash
# Vacuum and analyze
psql -U aframp_user -d aframp -c "VACUUM ANALYZE;"

# Reindex if needed
psql -U aframp_user -d aframp -c "REINDEX DATABASE aframp;"

# Update statistics
psql -U aframp_user -d aframp -c "ANALYZE;"
```

## Rollback Procedure

If deployment fails:

```bash
# Stop the service
sudo systemctl stop aframp-backend

# Restore database from backup
gunzip -c /var/backups/postgresql/aframp/aframp_backup_LATEST.sql.gz | \
  psql -U aframp_user -d aframp

# Revert to previous binary
cp target/release/Aframp-Backend.backup target/release/Aframp-Backend

# Start the service
sudo systemctl start aframp-backend
```

## Health Checks

### Application Health

```bash
# Check health endpoint
curl http://localhost:8000/health

# Expected response: OK
```

### Database Health

```bash
# Check database connectivity
psql -U aframp_user -d aframp -c "SELECT 1;"

# Check replication lag (if using replicas)
psql -U aframp_user -d aframp -c "
SELECT pg_last_wal_receive_lsn() - pg_last_wal_replay_lsn() AS lag;
"
```

## Support and Resources

- **Logs**: `/var/log/postgresql/` and `journalctl -u aframp-backend`
- **Configuration**: `.env.production`
- **Backups**: `/var/backups/postgresql/aframp/`
- **Service**: `systemctl status aframp-backend`

## Checklist

Before going live:

- [ ] Database created with secure credentials
- [ ] Migrations applied successfully
- [ ] Environment variables configured
- [ ] Release binary built and tested
- [ ] Systemd service installed and running
- [ ] Firewall configured
- [ ] SSL/TLS certificates installed
- [ ] Automated backups configured
- [ ] Monitoring tools set up
- [ ] Health checks passing
- [ ] Load testing completed
- [ ] Rollback procedure tested
- [ ] Documentation updated
- [ ] Team trained on operations

## Emergency Contacts

Document your emergency procedures and contacts here:

- **On-call Engineer**: [Contact Info]
- **Database Admin**: [Contact Info]
- **DevOps Team**: [Contact Info]
- **Escalation Path**: [Procedure]
