#!/bin/bash

# Aframp Backend Setup Script
# This script helps set up the development environment

set -e  # Exit on any error

echo "🚀 Setting up Aframp Backend Development Environment"

# Check if Rust is installed
if ! command -v rustc &> /dev/null; then
    echo "🦀 Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source ~/.cargo/env
else
    echo "✅ Rust is already installed"
fi

# Check if PostgreSQL is installed
if ! command -v psql &> /dev/null; then
    echo "🐘 Installing PostgreSQL..."
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        sudo apt update
        sudo apt install postgresql postgresql-contrib
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        brew install postgresql
    else
        echo "❌ Unsupported OS. Please install PostgreSQL manually."
        exit 1
    fi
else
    echo "✅ PostgreSQL is already installed"
fi

# Check if Redis is installed
if ! command -v redis-cli &> /dev/null; then
    echo "🧠 Installing Redis..."
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        sudo apt install redis-server
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        brew install redis
    else
        echo "❌ Unsupported OS. Please install Redis manually."
        exit 1
    fi
else
    echo "✅ Redis is already installed"
fi

# Start services
echo "🔄 Starting services..."
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    sudo systemctl start postgresql
    sudo systemctl start redis
elif [[ "$OSTYPE" == "darwin"* ]]; then
    brew services start postgresql
    brew services start redis
fi

# Create database
echo "📊 Creating database..."
sudo -u postgres createdb aframp 2>/dev/null || echo "✅ Database already exists"
sudo -u postgres createuser -s $USER 2>/dev/null || echo "✅ User already exists"

# Install sqlx CLI
if ! command -v sqlx &> /dev/null; then
    echo "🔧 Installing sqlx CLI..."
    cargo install --features postgres sqlx-cli --quiet
else
    echo "✅ sqlx CLI is already installed"
fi

# Run migrations
echo "📋 Running database migrations..."
DATABASE_URL=postgresql:///aframp sqlx migrate run

# Create .env file if it doesn't exist
if [ ! -f .env ]; then
    echo "📝 Creating .env file..."
    cp .env.example .env
    echo "✅ Created .env file. Please review and update as needed."
else
    echo "✅ .env file already exists"
fi

# Build the project
echo "🏗️ Building the project..."
cargo build

echo ""
echo "🎉 Setup complete!"
echo ""
echo "Next steps:"
echo "1. Review and update the .env file with your configuration"
echo "2. Run the server: cargo run"
echo "3. For development: cargo watch -x run (install cargo-watch first)"
echo ""
echo "For more information, check the README.md and QUICK_START.md files"