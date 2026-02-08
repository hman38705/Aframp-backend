#!/bin/bash

# Test Database Setup Script
set -e

echo "🧪 Setting up test database environment"

# Drop and recreate test database
echo "📊 Creating test database..."
dropdb aframp_test 2>/dev/null || true
createdb aframp_test

# Extract only the up migration (before -- migrate:down)
echo "📋 Running migrations on test database..."
sed '/-- migrate:down/,$d' migrations/20260122120000_create_core_schema.sql | psql -d aframp_test -v ON_ERROR_STOP=1

echo "✅ Core schema created!"

# Run payment schema migration
sed '/-- migrate:down/,$d' migrations/20260123040000_implement_payments_schema.sql | psql -d aframp_test -v ON_ERROR_STOP=1

echo "✅ Payment schema created!"

echo ""
echo "Test database: aframp_test"
echo "Connection string: postgresql:///aframp_test"
echo ""
echo "To use in tests, set: DATABASE_URL=postgresql:///aframp_test"
echo ""
echo "To connect: psql -d aframp_test"

