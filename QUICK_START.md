# Quick Start Guide

This guide will help you get the Aframp backend server up and running quickly.

## Prerequisites

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Install PostgreSQL**:
   ```bash
   # Ubuntu/Debian
   sudo apt update
   sudo apt install postgresql postgresql-contrib

   # macOS
   brew install postgresql

   # Start PostgreSQL service
   sudo systemctl start postgresql  # Linux
   brew services start postgresql   # macOS
   ```

3. **Install Redis**:
   ```bash
   # Ubuntu/Debian
   sudo apt install redis-server

   # macOS
   brew install redis

   # Start Redis service
   sudo systemctl start redis      # Linux
   brew services start redis       # macOS
   ```

## Quick Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/yourusername/aframp-backend.git
   cd aframp-backend
   ```

2. **Create database**:
   ```bash
   sudo -u postgres createdb aframp
   sudo -u postgres createuser -s $USER
   ```

3. **Configure environment**:
   ```bash
   cp .env.example .env
   ```

   Edit `.env` to match your setup:
   ```bash
   # For local development, these should work:
   STELLAR_NETWORK=testnet
   STELLAR_REQUEST_TIMEOUT=15
   STELLAR_MAX_RETRIES=3
   STELLAR_HEALTH_CHECK_INTERVAL=30
   REDIS_URL=redis://127.0.0.1:6379
   RUST_LOG=info
   ```

4. **Run the server**:
   ```bash
   cargo run
   ```

## Verify Setup

The server should start and show output like:
```
Starting Aframp backend service
Stellar client initialized successfully
Stellar Horizon is healthy - Response time: 123ms
=== Demo: Testing Stellar functionality ===
Account GCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3LDX does not exist (this is expected for test addresses)
Aframp backend service started successfully
```

## Test Endpoints

Once running, you can test:

1. **Health check**:
   ```bash
   curl http://localhost:8000/health
   ```

2. **Stellar account info** (if you have a Stellar account):
   ```bash
   curl "http://localhost:8000/api/stellar/account/GXXX..."
   ```

## Common Issues

### Database Connection Failed
```bash
# Check if PostgreSQL is running
sudo systemctl status postgresql

# Create database if missing
sudo -u postgres createdb aframp
```

### Redis Connection Failed
```bash
# Check if Redis is running
redis-cli ping

# If not running, start it:
sudo systemctl start redis  # Linux
brew services start redis   # macOS
```

### Rust Compilation Issues
```bash
# Update Rust
rustup update

# Clear build cache
cargo clean
```

## Development Workflow

1. **Run with auto-reload**:
   ```bash
   cargo install cargo-watch
   cargo watch -x run
   ```

2. **Run tests**:
   ```bash
   cargo test
   ```

3. **Check code quality**:
   ```bash
   cargo clippy
   cargo fmt --check
   ```

## Next Steps

1. Explore the API endpoints in `src/main.rs`
2. Check the database schema in `migrations/`
3. Review the Stellar integration in `src/chains/stellar/`
4. Look at the cache implementation in `src/cache/`

## Need Help?

- Check the full [README.md](README.md) for detailed documentation
- Review the [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines
- Look at the [STELLAR_INTEGRATION.md](STELLAR_INTEGRATION.md) for blockchain specifics