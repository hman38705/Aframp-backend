#!/bin/bash
# Database Backup Script

BACKUP_DIR="/var/backups/postgresql/aframp"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="$BACKUP_DIR/aframp_backup_$TIMESTAMP.sql.gz"

# Create backup
pg_dump -U aframp_user aframp | gzip > $BACKUP_FILE

# Keep only last 7 days of backups
find $BACKUP_DIR -name "aframp_backup_*.sql.gz" -mtime +7 -delete

echo "Backup completed: $BACKUP_FILE"
