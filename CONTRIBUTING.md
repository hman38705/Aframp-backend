# Contributing to Aframp Backend

## Error Handling Convention

_Issue #819: the codebase mixes `anyhow::Error` and `thiserror`-derived enums
across the domain/application boundary. This section establishes the
convention going forward._

**Domain layer** (`src/kyc/`, `src/services/`, `src/chains/`, repository and
engine code) — use `thiserror`-derived enums. Domain errors are things a
caller may need to match on (e.g. `InsufficientLiquidity`, `RateExpired`,
`WalletNotFound`). A typed enum keeps that possible; `anyhow::Error` erases
it. See `src/error.rs` (`AppErrorKind`, `DomainError`, `InfrastructureError`,
`ExternalError`, `ValidationError`) and `src/verification/engine.rs`
(`VerificationError`) for examples already following this pattern.

**API/handler layer** (`src/main.rs` route handlers, background worker
entry points) — use `anyhow::Error` (or `anyhow::Result`) for
context-rich wrapping of lower errors on their way to a log line or a
generic 500 response, via `.context("...")` / `.with_context(...)`. Handlers
that need specific HTTP status codes should convert the typed domain error
into `AppError` (which implements `IntoResponse`) rather than flattening it
into `anyhow` first.

**Rule of thumb:** if the error crosses a module boundary where the caller
might branch on the failure kind, it should be a typed `thiserror` enum. If
it's only ever going to be logged or turned into a generic failure response,
`anyhow` is fine.

### Code review checklist

- [ ] New domain/service code returns a `thiserror` enum, not `anyhow::Error`,
      in its public API.
- [ ] `anyhow` is only used at application boundaries (route handlers, `main.rs`
      wiring, worker `run()` loops), not threaded through domain logic.
- [ ] Domain errors that need a specific HTTP status are mapped through
      `AppError` / `AppErrorKind`, not stringified.
