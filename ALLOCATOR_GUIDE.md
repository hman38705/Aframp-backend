# Memory Allocator Guide for Aframp Backend

## Overview

The Aframp backend supports three memory allocators:
1. **jemalloc** (default for production) - Best for multi-threaded web servers
2. **mimalloc** - Fast alternative with good security features
3. **system allocator** - Default system allocator (used when no feature specified)

## Feature Flags

### Default Configuration (Production)
```toml
[features]
default = ["jemalloc", "database", "cache", "telemetry"]
```

### Building with Specific Allocators

```bash
# Build with jemalloc (recommended for production)
cargo build --release --features jemalloc

# Build with mimalloc
cargo build --release --features mimalloc

# Build with system allocator (development)
cargo build --release --no-default-features
```

## CI Testing

The CI pipeline includes matrix testing for all allocator variants:

| Job | Purpose | Runs On |
|-----|---------|---------|
| `allocator-matrix` | Compile and test with each allocator | PRs to develop/main |
| `allocator-benchmarks` | Performance comparison | Weekly schedule |
| `production-recommendation` | Generate deployment guidance | After benchmarks |

## Benchmark Results

Weekly benchmark results are available as CI artifacts. Key metrics tracked:

1. **Small allocations** (64-256 bytes): API keys, metadata
2. **Medium allocations** (1-4 KB): Transaction data, user profiles
3. **Large allocations** (64-256 KB): Batch processing, cached responses
4. **Concurrent allocations**: Simulating multi-threaded web server workload

## Production Recommendation

### 🥇 **jemalloc** - Recommended for Production
- **Best multi-threaded performance**: Optimized for concurrent web servers
- **Reduced memory fragmentation**: Critical for 24/7 uptime
- **Proven at scale**: Used by Redis, Firefox, Rust standard library (optional)
- **Extensive tuning**: Can be optimized for specific workload patterns

### 🥈 **mimalloc** - Good Alternative
- **Faster startup times**: Lower initialization overhead
- **Good security**: Includes guard pages and randomization
- **Simpler deployment**: Fewer platform-specific issues
- **Microsoft-backed**: Used in .NET runtime and other Microsoft projects

### 🥉 **system allocator** - Development/Testing
- **No dependencies**: Simplest setup
- **Better debugging**: Easier to profile with standard tools
- **Consistent behavior**: Works on all platforms without extra dependencies

## Implementation Details

### Code Location
- `src/allocator.rs` - Allocator configuration and detection
- `tests/allocator_benchmarks.rs` - Performance benchmarks
- `.github/workflows/allocator-tests.yml` - CI testing

### Verification
Check which allocator is active:
```bash
cargo run --example allocator_check --features jemalloc
```

Example output:
```
=== Allocator Verification ===
Active allocator: jemalloc
=== Allocation Test ===
Small allocation (1KB): 0x7f8b5c000000
Medium allocation (1MB): 0x7f8b58000000
Large allocation (10MB): 0x7f8b40000000
```

## Docker Configuration

### Production Dockerfile
```dockerfile
# Build with jemalloc for production
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --features jemalloc

# Runtime image
FROM debian:bullseye-slim
COPY --from=builder /app/target/release/aframp-backend /usr/local/bin/
CMD ["aframp-backend"]
```

### Development Dockerfile
```dockerfile
# Build with system allocator for development
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --no-default-features

# Runtime image
FROM debian:bullseye-slim
COPY --from=builder /app/target/release/aframp-backend /usr/local/bin/
CMD ["aframp-backend"]
```

## Monitoring & Metrics

### Memory Usage Tracking
```rust
use aframp_backend::allocator;

// Get allocator name for metrics
let allocator_name = allocator::get_allocator_name();

// Get allocator statistics (if available)
if let Some(stats) = allocator::get_allocator_stats() {
    println!("Allocator stats: {}", stats);
}
```

### Prometheus Metrics
Consider adding these metrics for production monitoring:
- `allocator_active_bytes` - Currently allocated memory
- `allocator_allocated_total` - Total allocations made
- `allocator_fragmentation_ratio` - Memory fragmentation level

## Troubleshooting

### Common Issues

1. **jemalloc not available on Windows/MSVC**
   - Use `mimalloc` or `system` allocator instead
   - jemalloc requires Unix-like platforms

2. **Performance regression after allocator change**
   - Run benchmarks to verify: `cargo test --test allocator_benchmarks`
   - Check memory usage patterns match allocator strengths

3. **Build failures with allocator features**
   - Ensure dependencies are properly specified in Cargo.toml
   - Check platform compatibility

### Debugging
```bash
# Run with detailed allocator logging (jemalloc)
MALLOC_CONF="stats_print:true" ./target/release/aframp-backend

# Run with mimalloc verbose mode
MIMALLOC_VERBOSE=1 ./target/release/aframp-backend
```

## Performance Tuning

### jemalloc Tuning
```bash
# Set jemalloc configuration via environment
export MALLOC_CONF="narenas:4,dirty_decay_ms:10000,muzzy_decay_ms:10000"
./target/release/aframp-backend
```

### mimalloc Tuning
```bash
# Enable secure mode with guard pages
export MIMALLOC_SECURE=1
# Set page reservation size
export MIMALLOC_LARGE_OS_PAGES=1
./target/release/aframp-backend
```

## References

- [jemalloc documentation](http://jemalloc.net/jemalloc.3.html)
- [mimalloc GitHub](https://github.com/microsoft/mimalloc)
- [Rust performance book - Allocators](https://nnethercote.github.io/perf-book/heap-allocations.html)
- [Choosing a memory allocator](https://www.youtube.com/watch?v=kSW5qP8tHyo)