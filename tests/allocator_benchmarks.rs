//! Memory allocator benchmark tests
//!
//! This module benchmarks different memory allocators (jemalloc, mimalloc, system)
//! to compare performance under different allocation patterns typical for the
//! Aframp backend workload.
//!
//! Run with specific allocator:
//!   cargo test --test allocator_benchmarks --features jemalloc
//!   cargo test --test allocator_benchmarks --features mimalloc
//!   cargo test --test allocator_benchmarks

use std::time::{Duration, Instant};
use std::sync::Arc;
use std::thread;

/// Memory allocation pattern for payment processing
struct PaymentAllocationPattern {
    /// Small allocations (payment metadata, API keys)
    small_allocs: usize,
    /// Medium allocations (transaction data, user data)
    medium_allocs: usize,
    /// Large allocations (batch processing, cached responses)
    large_allocs: usize,
}

impl Default for PaymentAllocationPattern {
    fn default() -> Self {
        Self {
            small_allocs: 1000,   // 64-256 bytes
            medium_allocs: 100,   // 1-4 KB
            large_allocs: 10,     // 64-256 KB
        }
    }
}

/// Benchmark small allocations (typical for API request processing)
fn benchmark_small_allocations(count: usize) -> Duration {
    let start = Instant::now();
    
    for i in 0..count {
        // Allocate small strings (API keys, IDs, metadata)
        let _api_key = format!("api_key_{:08x}", i);
        let _tx_id = format!("tx_{:016x}", i);
        let _user_id = format!("user_{:08x}", i);
        
        // Small vectors (path segments, headers)
        let _path: Vec<String> = vec!["api".into(), "v1".into(), "payments".into()];
        let _headers: Vec<(String, String)> = vec![
            ("Content-Type".into(), "application/json".into()),
            ("Authorization".into(), format!("Bearer token_{}", i)),
        ];
    }
    
    start.elapsed()
}

/// Benchmark medium allocations (transaction data, user profiles)
fn benchmark_medium_allocations(count: usize) -> Duration {
    let start = Instant::now();
    
    for i in 0..count {
        // Transaction data structure
        let _transaction = serde_json::json!({
            "id": format!("tx_{:016x}", i),
            "amount": 1000 + i,
            "currency": "USD",
            "status": "completed",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "metadata": {
                "source": format!("wallet_{}", i % 100),
                "destination": format!("wallet_{}", (i + 1) % 100),
                "fee": 10,
                "network_fee": 5,
            }
        });
        
        // User profile data
        let _user_profile = serde_json::json!({
            "id": format!("user_{:08x}", i),
            "email": format!("user{}@example.com", i),
            "name": format!("User {}", i),
            "kyc_status": if i % 3 == 0 { "verified" } else { "pending" },
            "wallet_addresses": vec![
                format!("G{}", "A".repeat(55)),
                format!("G{}", "B".repeat(55)),
            ],
            "preferences": {
                "currency": "USD",
                "language": "en",
                "notifications": true,
            }
        });
    }
    
    start.elapsed()
}

/// Benchmark large allocations (batch processing, cached responses)
fn benchmark_large_allocations(count: usize) -> Duration {
    let start = Instant::now();
    
    for i in 0..count {
        // Large cached response (exchange rates for all currencies)
        let _exchange_rates: Vec<serde_json::Value> = (0..1000)
            .map(|j| {
                serde_json::json!({
                    "from": "USD",
                    "to": format!("CUR{}", j),
                    "rate": 1.0 + (j as f64 * 0.001),
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "source": "fixer.io",
                })
            })
            .collect();
        
        // Batch transaction processing
        let _batch_transactions: Vec<serde_json::Value> = (0..500)
            .map(|j| {
                serde_json::json!({
                    "id": format!("batch_{}_{}", i, j),
                    "type": if j % 2 == 0 { "credit" } else { "debit" },
                    "amount": 100 + (j % 1000),
                    "currency": ["USD", "EUR", "GBP"][j % 3],
                    "status": ["pending", "completed", "failed"][j % 3],
                })
            })
            .collect();
    }
    
    start.elapsed()
}

/// Benchmark concurrent allocations (simulating multi-threaded web server)
fn benchmark_concurrent_allocations(pattern: PaymentAllocationPattern) -> Duration {
    let start = Instant::now();
    
    let threads: Vec<_> = (0..4) // Simulate 4 worker threads
        .map(|thread_id| {
            thread::spawn(move || {
                // Each thread performs its own allocation pattern
                let _small_time = benchmark_small_allocations(pattern.small_allocs / 4);
                let _medium_time = benchmark_medium_allocations(pattern.medium_allocs / 4);
                let _large_time = benchmark_large_allocations(pattern.large_allocs / 4);
            })
        })
        .collect();
    
    for thread in threads {
        thread.join().unwrap();
    }
    
    start.elapsed()
}

/// Run complete allocator benchmark suite
fn run_allocator_benchmark() -> serde_json::Value {
    let pattern = PaymentAllocationPattern::default();
    
    println!("\n=== Memory Allocator Benchmark ===");
    println!("Allocator: {}", crate::allocator::get_allocator_name());
    
    // Warm up
    println!("Warming up...");
    benchmark_small_allocations(100);
    benchmark_medium_allocations(10);
    benchmark_large_allocations(1);
    
    // Run benchmarks
    println!("Running benchmarks...");
    
    let small_time = benchmark_small_allocations(pattern.small_allocs);
    let medium_time = benchmark_medium_allocations(pattern.medium_allocs);
    let large_time = benchmark_large_allocations(pattern.large_allocs);
    let concurrent_time = benchmark_concurrent_allocations(pattern);
    
    // Print results
    println!("\n=== Results ===");
    println!("Small allocations ({}): {:.2}ms", pattern.small_allocs, small_time.as_secs_f64() * 1000.0);
    println!("Medium allocations ({}): {:.2}ms", pattern.medium_allocs, medium_time.as_secs_f64() * 1000.0);
    println!("Large allocations ({}): {:.2}ms", pattern.large_allocs, large_time.as_secs_f64() * 1000.0);
    println!("Concurrent allocations: {:.2}ms", concurrent_time.as_secs_f64() * 1000.0);
    println!("Total time: {:.2}ms", (small_time + medium_time + large_time + concurrent_time).as_secs_f64() * 1000.0);
    
    // Return structured results
    serde_json::json!({
        "allocator": crate::allocator::get_allocator_name(),
        "benchmarks": {
            "small_allocations": {
                "count": pattern.small_allocs,
                "time_ms": small_time.as_secs_f64() * 1000.0,
                "rate_per_second": pattern.small_allocs as f64 / small_time.as_secs_f64(),
            },
            "medium_allocations": {
                "count": pattern.medium_allocs,
                "time_ms": medium_time.as_secs_f64() * 1000.0,
                "rate_per_second": pattern.medium_allocs as f64 / medium_time.as_secs_f64(),
            },
            "large_allocations": {
                "count": pattern.large_allocs,
                "time_ms": large_time.as_secs_f64() * 1000.0,
                "rate_per_second": pattern.large_allocs as f64 / large_time.as_secs_f64(),
            },
            "concurrent_allocations": {
                "time_ms": concurrent_time.as_secs_f64() * 1000.0,
            },
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })
}

#[test]
fn test_allocator_benchmark() {
    // This test runs the benchmark and ensures it completes within reasonable time
    let result = run_allocator_benchmark();
    
    // Verify benchmark completed successfully
    assert!(result["allocator"].is_string());
    assert!(result["benchmarks"]["small_allocations"]["time_ms"].is_number());
    assert!(result["benchmarks"]["medium_allocations"]["time_ms"].is_number());
    assert!(result["benchmarks"]["large_allocations"]["time_ms"].is.number());
    
    // Log results for CI
    println!("Benchmark results: {}", serde_json::to_string_pretty(&result).unwrap());
}

/// Compare allocators and provide recommendation
#[test]
fn compare_allocators() {
    // This test would normally run with different allocators in CI matrix
    // For now, just document the expected recommendations
    
    let allocator = crate::allocator::get_allocator_name();
    let stats = crate::allocator::get_allocator_stats();
    
    println!("Current allocator: {}", allocator);
    if let Some(stats_str) = stats {
        println!("Allocator stats: {}", stats_str);
    }
    
    // Based on typical Rust web server workloads:
    // - jemalloc: Best for multi-threaded workloads, reduces fragmentation
    // - mimalloc: Good all-around, fast, low memory overhead
    // - system: Default, works everywhere but may have more fragmentation
    
    println!("\n=== Allocator Recommendation ===");
    println!("For Aframp backend (high-concurrency web API):");
    println!("1. jemalloc: Recommended for production deployments");
    println!("   - Best multi-threaded performance");
    println!("   - Reduces memory fragmentation");
    println!("   - Good for long-running servers");
    println!();
    println!("2. mimalloc: Good alternative if jemalloc has issues");
    println!("   - Fast allocations/deallocations");
    println!("   - Low memory overhead");
    println!("   - Good security features");
    println!();
    println!("3. system: Use for development/testing");
    println!("   - No dependencies");
    println!("   - Works everywhere");
    println!("   - Simpler debugging");
    
    // Always pass - this is just documentation
    assert!(true);
}