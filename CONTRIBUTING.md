# Contributing to Aframp Backend

Thank you for your interest in contributing to Aframp Backend! This guide will help you get started with contributing to our African crypto onramp/offramp payment infrastructure.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Architecture](#project-architecture)
- [Development Workflow](#development-workflow)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Database Changes](#database-changes)
- [Pull Request Process](#pull-request-process)
- [Blockchain Integration](#blockchain-integration)
- [Security Considerations](#security-considerations)
- [Common Tasks](#common-tasks)
- [Getting Help](#getting-help)

## Code of Conduct

We are committed to providing a welcoming and inclusive environment. Please be respectful and professional in all interactions.

## Getting Started

### Prerequisites

- **Rust 1.75+**: Install via [rustup](https://rustup.rs/)
- **PostgreSQL 14+**: Database for persistence
- **Redis 6+**: For caching and rate limiting
- **Git**: Version control
- **Docker** (optional): For containerized development

### First-Time Setup

1. **Fork and clone the repository**

```bash
git clone https://github.com/YOUR_USERNAME/aframp-backend.git
cd aframp-backend
```

2. **Install dependencies**

```bash
# Install Rust toolchain
rustup update

# Install development tools
rustup component add clippy rustfmt
cargo install cargo-watch cargo-audit sqlx-cli
```

3. **Set up environment**

```bash
cp .env.example .env
# Edit .env with your local configuration
```

4. **Start services**

```bash
# Using Docker Compose (recommended)
docker-compose up -d postgres redis

# Or install PostgreSQL and Redis locally
```

5. **Run database migrations**

```bash
sqlx migrate run
```

6. **Build and test**

```bash
cargo build --features database
cargo test --features database
```

## Development Setup

### Environment Variables

Key environment variables you need to configure:

```bash
# Stellar Network
STELLAR_NETWORK=testnet  # or mainnet
STELLAR_REQUEST_TIMEOUT=10
STELLAR_MAX_RETRIES=3

# Logging
RUST_LOG=debug  # Use debug for development, info for production

# Database (for database feature)
DATABASE_URL=postgresql://user:password@localhost/aframp

# Redis (for caching feature)
REDIS_URL=redis://localhost:6379
```

### Running the Application

```bash
# Development mode with auto-reload
cargo watch -x "run --features database"

# Standard run
cargo run --features database

# Production build
cargo build --release --features database
./target/release/Bitmesh-backend
```

## Project Architecture

### Directory Structure

```
src/
├── chains/              # Blockchain integrations
│   └── stellar/         # Stellar blockchain (CNGN stablecoin)
│       ├── client.rs    # Horizon API client
│       ├── config.rs    # Network configuration
│       ├── errors.rs    # Error types
│       ├── types.rs     # Data structures
│       └── tests.rs     # Unit tests
├── database/            # Data persistence layer
│   ├── *_repository.rs  # Repository pattern implementations
│   ├── error.rs         # Database error types
│   ├── transaction.rs   # Transaction management
│   └── mod.rs           # Module exports
├── middleware/          # HTTP middleware
│   ├── logging.rs       # Request/response logging
│   └── error.rs         # Error handling
├── error.rs             # Global error types
├── lib.rs               # Library exports (includes Soroban contract)
├── logging.rs           # Logging configuration
└── main.rs              # Application entry point

migrations/              # Database migrations
contracts/               # Soroban smart contracts
examples/                # Example code
```

### Key Components

1. **Stellar Integration**: Primary blockchain for CNGN stablecoin
2. **Database Layer**: Repository pattern with SQLx
3. **Soroban Contracts**: Smart contracts for escrow functionality
4. **Middleware**: Logging, error handling, rate limiting

## Development Workflow

### 1. Create a Feature Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/bug-description
```

### Branch Naming Conventions

- `feature/`: New features
- `fix/`: Bug fixes
- `refactor/`: Code refactoring
- `docs/`: Documentation updates
- `test/`: Test additions or updates
- `chore/`: Maintenance tasks

### 2. Make Your Changes

Follow the coding standards and write tests for your changes.

### 3. Test Your Changes

```bash
# Run all tests
cargo test --features database

# Run specific test
cargo test --features database test_name

# Run with output
cargo test --features database -- --nocapture

# Run clippy for linting
cargo clippy --features database -- -D warnings

# Format code
cargo fmt
```

### 4. Commit Your Changes

Use clear, descriptive commit messages:

```bash
git add .
git commit -m "feat: add CNGN balance checking endpoint

- Implement balance retrieval from Stellar
- Add input validation
- Include unit tests
- Update API documentation"
```

**Commit Message Format**:
- `feat:` New feature
- `fix:` Bug fix
- `refactor:` Code refactoring
- `docs:` Documentation changes
- `test:` Test additions/updates
- `chore:` Maintenance tasks

### 5. Push and Create Pull Request

```bash
git push origin feature/your-feature-name
```

Then create a Pull Request on GitHub.

## Coding Standards

### Rust Conventions

1. **Follow Rust naming conventions**
   - `snake_case` for functions, variables, modules
   - `PascalCase` for types, traits, enums
   - `SCREAMING_SNAKE_CASE` for constants

2. **Error handling**
   - Use `Result<T, E>` for fallible operations
   - Create custom error types using `thiserror`
   - Provide context in error messages

3. **Async/await**
   - Use `async fn` for I/O operations
   - Avoid blocking operations in async context
   - Use `tokio::spawn` for concurrent tasks

4. **Documentation**
   - Document public APIs with `///` comments
   - Include examples in doc comments
   - Keep comments up-to-date

### Code Style

```rust
// Good: Clear, documented function
/// Fetches account details from Stellar Horizon API
///
/// # Arguments
/// * `address` - Valid 56-character Stellar address
///
/// # Returns
/// Account information including balances and sequence number
///
/// # Errors
/// Returns error if account doesn't exist or network fails
pub async fn get_account(&self, address: &str) -> StellarResult<StellarAccountInfo> {
    // Validate input
    if !is_valid_stellar_address(address) {
        return Err(StellarError::invalid_address(address));
    }
    
    // Implementation...
}

// Bad: No documentation, unclear purpose
pub async fn get_acc(&self, a: &str) -> Result<AccInfo, Error> {
    // ...
}
```

### Security Best Practices

- **Never commit secrets**: Use environment variables
- **Validate all inputs**: Check addresses, amounts, etc.
- **Use prepared statements**: SQLx prevents SQL injection
- **No private keys in code**: Non-custodial design only
- **Rate limit operations**: Protect against abuse

## Testing Guidelines

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_stellar_address() {
        let valid = "GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
        assert!(is_valid_stellar_address(valid));
    }

    #[tokio::test]
    async fn test_account_fetch() {
        let config = StellarConfig::default();
        let client = StellarClient::new(config).unwrap();
        
        // Test implementation
    }
}
```

### Test Requirements

- **Unit tests**: Test individual functions in isolation
- **Integration tests**: Test component interactions
- **Edge cases**: Test boundary conditions and error paths
- **Documentation**: Explain what each test validates

### Running Tests

```bash
# All tests
cargo test --features database

# Specific module
cargo test --features database stellar::

# With logging
RUST_LOG=debug cargo test --features database -- --nocapture

# Integration tests (requires testnet)
cargo test --features database,integration
```

## Database Changes

### Creating Migrations

```bash
# Create a new migration
sqlx migrate add descriptive_name

# Edit the generated SQL file in migrations/
# Format: YYYYMMDDHHMMSS_descriptive_name.sql
```

### Migration Guidelines

1. **Idempotent**: Use `IF NOT EXISTS` where applicable
2. **Reversible**: Include rollback logic when possible
3. **Tested**: Test migrations on a copy of production data
4. **Documented**: Explain complex changes in comments
5. **Performance**: Consider impact on large tables

### Example Migration

```sql
-- Create trustlines table
CREATE TABLE IF NOT EXISTS trustlines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_address VARCHAR(56) NOT NULL,
    asset_code VARCHAR(12) NOT NULL,
    asset_issuer VARCHAR(56) NOT NULL,
    limit_amount TEXT,
    balance TEXT NOT NULL DEFAULT '0',
    is_authorized BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(wallet_address, asset_code, asset_issuer)
);

-- Add indexes
CREATE INDEX idx_trustlines_wallet ON trustlines(wallet_address);
CREATE INDEX idx_trustlines_asset ON trustlines(asset_code, asset_issuer);
```

### Running Migrations

```bash
# Run all pending migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert

# Check migration status
sqlx migrate info
```

## Pull Request Process

### Before Submitting

- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] New tests added for new functionality
- [ ] Code formatted with `cargo fmt`
- [ ] Clippy checks pass with no warnings
- [ ] Documentation updated
- [ ] Commit messages are clear

### PR Guidelines

1. **Title**: Clear, descriptive title summarizing the change
2. **Description**: Explain what, why, and how
3. **References**: Link related issues
4. **Screenshots**: Include for UI changes
5. **Breaking changes**: Clearly documented

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
Describe testing done

## Checklist
- [ ] Tests pass
- [ ] Code formatted
- [ ] Documentation updated
- [ ] No new warnings
```

### Review Process

1. Automated CI/CD runs tests and checks
2. At least one maintainer reviews code
3. Address feedback and update PR
4. Maintainer approves and merges

## Blockchain Integration

### Adding New Blockchain Support

1. Create module in `src/chains/`
2. Implement common traits
3. Add configuration
4. Write comprehensive tests
5. Update documentation

### Stellar-Specific Guidelines

- Use testnet for development
- Validate addresses before API calls
- Handle rate limits gracefully
- Cache frequently accessed data
- Log all blockchain operations

### CNGN Stablecoin Operations

When working with CNGN:
- Verify trustline exists before transfers
- Validate asset issuer address
- Use string types for amounts (avoid float precision issues)
- Handle XLM fee requirements

## Security Considerations

### Critical Security Rules

1. **No private keys**: Never store or log private keys
2. **Input validation**: Validate all user inputs
3. **Rate limiting**: Implement for all public endpoints
4. **SQL injection**: Use SQLx parameterized queries only
5. **Secrets management**: Use environment variables
6. **Audit logs**: Log sensitive operations

### Reporting Security Issues

**Do NOT open public issues for security vulnerabilities.**

Email security concerns to: security@aframp.com

Include:
- Description of vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

## Common Tasks

### Adding a New API Endpoint

1. Define handler in appropriate module
2. Add route in router configuration
3. Implement business logic
4. Add input validation
5. Write unit and integration tests
6. Update API documentation

### Adding a New Repository

1. Create file in `src/database/`
2. Define repository trait
3. Implement with SQLx
4. Add error handling
5. Write tests with test database
6. Update module exports

### Updating Dependencies

```bash
# Check for outdated dependencies
cargo outdated

# Update dependencies
cargo update

# Test after updating
cargo test --features database
cargo clippy --features database
```

### Security Audit

```bash
# Run security audit
cargo audit

# Fix vulnerabilities
cargo audit fix
```

## Getting Help

### Resources

- **Documentation**: Check README.md and STELLAR_INTEGRATION.md
- **API Docs**: Run `cargo doc --open --features database`
- **Issues**: Search existing issues before creating new ones
- **Discussions**: Use GitHub Discussions for questions

### Communication Channels

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Questions and general discussion
- **Pull Requests**: Code review and technical discussion

### Development Tips

1. Use `RUST_LOG=debug` for verbose logging
2. Run `cargo watch` for automatic rebuilds
3. Use `cargo clippy` to catch common mistakes
4. Check `cargo doc` for inline documentation
5. Review existing code for patterns and conventions

## Recognition

Contributors are recognized in:
- GitHub contributors page
- Release notes for significant contributions
- Special mentions for major features

---

**Thank you for contributing to Aframp Backend!**

Together, we're building the future of African crypto infrastructure powered by CNGN stablecoin and Rust.
