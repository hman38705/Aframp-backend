#553 [Codebase Audit] Reduce panic-prone calls in src/metrics/mod.rs
Repo Avatar
kellymusk/Aframp-backend
Problem
src/metrics/mod.rs contains 85 panic-prone calls (unwrap, expect, or panic!).

Evidence
src/metrics/mod.rs:41 — .expect("encoding metrics failed");
src/metrics/mod.rs:42 — String::from_utf8(buf).expect("metrics output is not valid UTF-8")
src/metrics/mod.rs:57 — HTTP_REQUESTS_TOTAL.get().expect("metrics not initialised")
src/metrics/mod.rs:63 — .expect("metrics not initialised")
src/metrics/mod.rs:69 — .expect("metrics not initialised")
Proposed fix
Replace non-essential unwrap/expect usages with typed error propagation and contextual logging. Keep explicit panics only where unrecoverable invariants are well-documented.

Acceptance criteria
All avoidable panic-prone calls in this file are removed or justified with comments/tests.
Error paths return typed errors and preserve observability context.
Existing tests pass (or new tests cover changed paths).

#571 [Codebase Audit] Reduce panic-prone calls in tests/cache_integration_test.rs
Repo Avatar
kellymusk/Aframp-backend
Problem
tests/cache_integration_test.rs contains 30 panic-prone calls (unwrap, expect, or panic!).

Evidence
tests/cache_integration_test.rs:24 — .expect("Failed to init cache pool");
tests/cache_integration_test.rs:29 — let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
tests/cache_integration_test.rs:33 — .expect("Failed to init DB pool")
tests/cache_integration_test.rs:48 — .unwrap();
tests/cache_integration_test.rs:51 — let cached_rate = repo.get_current_rate("AFRI", "USD").await.unwrap();
Proposed fix
Replace non-essential unwrap/expect usages with typed error propagation and contextual logging. Keep explicit panics only where unrecoverable invariants are well-documented.

Acceptance criteria
All avoidable panic-prone calls in this file are removed or justified with comments/tests.
Error paths return typed errors and preserve observability context.
Existing tests pass (or new tests cover changed paths).

#576 [Codebase Audit] Reduce panic-prone calls in tests/mint_burn_integration.rs
Repo Avatar
kellymusk/Aframp-backend
Problem
tests/mint_burn_integration.rs contains 27 panic-prone calls (unwrap, expect, or panic!).

Evidence
tests/mint_burn_integration.rs:41 — .expect("DATABASE_URL must be set for integration tests");
tests/mint_burn_integration.rs:42 — PgPool::connect(&url).await.expect("db pool")
tests/mint_burn_integration.rs:47 — Arc::new(MintBurnMetrics::new(®istry).expect("metrics"))
tests/mint_burn_integration.rs:91 — .expect("create processed_events");
tests/mint_burn_integration.rs:105 — .expect("create ledger_cursor");
Proposed fix
Replace non-essential unwrap/expect usages with typed error propagation and contextual logging. Keep explicit panics only where unrecoverable invariants are well-documented.

Acceptance criteria
All avoidable panic-prone calls in this file are removed or justified with comments/tests.
Error paths return typed errors and preserve observability context.
Existing tests pass (or new tests cover changed paths).