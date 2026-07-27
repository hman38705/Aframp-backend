//! Shared integration-test support.
//!
//! `TestDatabase` spins up a disposable Postgres container via `testcontainers`,
//! runs the project's migrations against it, and tears the container down when
//! dropped. This removes the need for `docker-compose up` / `setup-test-db.sh`
//! before running `cargo test`.
//!
//! ```ignore
//! let db = common::TestDatabase::new().await;
//! let pool = db.pool();
//! // ... use pool in the test ...
//! // container stops automatically when `db` is dropped
//! ```

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;

pub struct TestDatabase {
    // Held only to keep the container alive for the lifetime of `TestDatabase`;
    // `None` when running against a pre-existing database (fallback mode).
    _container: Option<ContainerAsync<Postgres>>,
    pool: PgPool,
}

impl TestDatabase {
    /// Starts a fresh Postgres container and applies all migrations.
    ///
    /// Falls back to `DATABASE_URL` / `TEST_DATABASE_URL` when
    /// `TESTCONTAINERS_DISABLE=1` is set, so CI or local setups that still
    /// prefer a pre-running database keep working.
    pub async fn new() -> Self {
        if std::env::var("TESTCONTAINERS_DISABLE").is_ok() {
            let database_url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| {
                    "postgresql://postgres:postgres@localhost:5432/aframp_test".to_string()
                });
            let pool = PgPoolOptions::new()
                .max_connections(5)
                .connect(&database_url)
                .await
                .expect("failed to connect to fallback test database");
            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .expect("failed to run migrations against fallback test database");
            return Self {
                _container: None,
                pool,
            };
        }

        let container = Postgres::default()
            .start()
            .await
            .expect("failed to start postgres testcontainer");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("failed to get mapped postgres port");
        let database_url = format!("postgresql://postgres:postgres@127.0.0.1:{port}/postgres");

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("failed to connect to postgres testcontainer");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("failed to run migrations against testcontainer database");

        Self {
            _container: Some(container),
            pool,
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
