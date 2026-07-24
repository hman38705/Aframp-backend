# Fixing Duplicate Dependencies in Cargo.lock

## Identified Duplicates

The following duplicate dependencies were found in `Cargo.lock`:

### 1. `rand` crate - multiple versions
- `rand = "0.7.0"` - old version
- `rand = "0.8.6"` - intermediate version  
- `rand = "0.9.2"` - current version specified in Cargo.toml

### 2. `http` crate - multiple versions
- `http = "0.2.12"` - old version
- `http = "1.4.0"` - current version (Cargo.toml specifies 1.0)

### 3. `thiserror` crate - multiple versions
- `thiserror = "1.0.69"` - old version
- `thiserror = "2.0.18"` - current version specified in Cargo.toml

## Steps to Fix Duplicates

### Option 1: Manual Resolution (Recommended)

1. Update `Cargo.toml` to pin exact versions:

```toml
[dependencies]
rand = { version = "0.9.2", features = ["std"], optional = true }
http = { version = "1.4.0", optional = true }
```

2. Remove unused dependency versions:

```bash
# This will update Cargo.lock and resolve duplicates
cargo update
```

### Option 2: Using cargo-deny (For CI Enforcement)

1. Install cargo-deny:
```bash
cargo install cargo-deny
```

2. Create `deny.toml` configuration:
```toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"
yanked = "deny"

[bans]
multiple-versions = "deny"
wildcards = "deny"

[licenses]
unlicensed = "deny"
copyleft = "deny"
```

3. Run cargo-deny to check for duplicates:
```bash
cargo deny check bans
```

### Option 3: Update Dependencies

Run cargo update to resolve to latest compatible versions:
```bash
cargo update
cargo tree --duplicates  # Verify duplicates are resolved
```

## Adding cargo-deny to CI

Add this step to your CI workflow:

```yaml
- name: Check for duplicate dependencies
  run: |
    cargo install cargo-deny
    cargo deny check bans
```

## Current Cargo.toml Issues

1. **rand version specifier**: Currently `"0.9"` which resolves to `0.9.2` - this is fine
2. **http version specifier**: Currently `"1.0"` but lock has `1.4.0` - update to `"1.4"`

## Recommended Actions

1. Update `Cargo.toml` with exact version pins
2. Run `cargo update` to regenerate `Cargo.lock`
3. Add `cargo deny` to CI pipeline to prevent future duplicates
4. Run `cargo audit` to check for security vulnerabilities

## Running cargo audit

```bash
cargo install cargo-audit
cargo audit
```

If vulnerabilities are found, they should be addressed by updating affected dependencies.