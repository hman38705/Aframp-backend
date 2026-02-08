#!/bin/bash

# Run backend with test database
echo "🧪 Starting backend with test database..."
echo ""
echo "Database: aframp_test"
echo "Port: 8001"
echo ""

# Load test environment
export $(cat .env.test | xargs)

# Run the server
cargo run
