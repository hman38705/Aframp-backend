//! Example program to verify which allocator is active

use aframp_backend::allocator;

fn main() {
    println!("=== Allocator Verification ===");
    println!("Active allocator: {}", allocator::get_allocator_name());
    
    if let Some(stats) = allocator::get_allocator_stats() {
        println!("Allocator stats: {}", stats);
    }
    
    // Test allocation to verify allocator is working
    println!("\n=== Allocation Test ===");
    
    // Test small allocation
    let small_vec: Vec<u8> = vec![0; 1024]; // 1KB
    println!("Small allocation (1KB): {:?}", small_vec.as_ptr());
    
    // Test medium allocation
    let medium_vec: Vec<u8> = vec![0; 1024 * 1024]; // 1MB
    println!("Medium allocation (1MB): {:?}", medium_vec.as_ptr());
    
    // Test large allocation
    let large_vec: Vec<u8> = vec![0; 10 * 1024 * 1024]; // 10MB
    println!("Large allocation (10MB): {:?}", large_vec.as_ptr());
    
    println!("\n=== Allocator Information ===");
    
    #[cfg(all(feature = "jemalloc", not(target_env = "msvc")))]
    println!("jemalloc is ENABLED (non-MSVC target)");
    
    #[cfg(not(all(feature = "jemalloc", not(target_env = "msvc"))))]
    println!("jemalloc is DISABLED");
    
    #[cfg(feature = "mimalloc")]
    println!("mimalloc is ENABLED");
    
    #[cfg(not(feature = "mimalloc"))]
    println!("mimalloc is DISABLED");
    
    println!("\n✅ Allocator verification complete");
    
    // Provide recommendation
    println!("\n=== Production Recommendation ===");
    let alloc_name = allocator::get_allocator_name();
    match alloc_name {
        "jemalloc" => {
            println!("✅ Running with jemalloc - recommended for production!");
            println!("   - Best for multi-threaded web servers");
            println!("   - Reduces memory fragmentation");
            println!("   - Proven in production at scale");
        }
        "mimalloc" => {
            println!("✓ Running with mimalloc - good alternative");
            println!("   - Fast allocations");
            println!("   - Good security features");
            println!("   - Lower memory overhead");
        }
        "system" => {
            println!("⚠ Running with system allocator - suitable for development");
            println!("   Consider enabling jemalloc or mimalloc for production:");
            println!("   - Add '--features jemalloc' to build command");
            println!("   - Or add '--features mimalloc' to build command");
        }
        _ => {
            println!("❓ Unknown allocator: {}", alloc_name);
        }
    }
}