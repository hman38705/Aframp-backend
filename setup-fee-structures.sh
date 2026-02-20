#!/bin/bash
# Setup fee structures for Aframp backend
# This script runs the migration and seeds initial fee configurations

set -e

echo "🚀 Setting up fee structures..."

# Load environment variables
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

# Default to test database if not specified
DATABASE_URL=${DATABASE_URL:-"postgresql://postgres:postgres@localhost/aframp_test"}

echo "📊 Database: $DATABASE_URL"

# Run migration
echo "⚙️  Running migration..."
sqlx migrate run --database-url "$DATABASE_URL"

# Seed fee structures
echo "🌱 Seeding fee structures..."
psql "$DATABASE_URL" -f db/seed_fee_structures.sql

echo "✅ Fee structures setup complete!"
echo ""
echo "📋 Summary:"
psql "$DATABASE_URL" -c "SELECT COUNT(*) as total_fee_structures FROM fee_structures WHERE is_active = true;"
echo ""
echo "💡 To view all fee structures:"
echo "   psql $DATABASE_URL -c \"SELECT transaction_type, payment_provider, payment_method, min_amount, max_amount FROM fee_structures WHERE is_active = true ORDER BY transaction_type, min_amount;\""
