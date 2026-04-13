# EDUstell

EDUstell is a modular-monolith Rust backend for education savings, tuition payouts, compliance, and auditability.

## Current Scope

The repository now includes Phase 1 plus the first Phase 2 auth/RBAC slice:

- Cargo workspace layout
- Axum API bootstrap
- `/health` and `/ready`
- typed environment configuration
- tracing setup
- PostgreSQL pool setup with SQLx
- SQLx migration runner
- initial `users`, `audit_logs`, and auth-session tables
- auth register/login/refresh/logout/me routes
- JWT access tokens and hashed refresh tokens
- current-user extractor and role-based authorization helper

Business modules like plans, vaults, milestones, payouts, and compliance workflows are still intentionally not implemented.

## Workspace

- `apps/api`: HTTP bootstrap, router, config, telemetry, middleware/extractor placeholders
- `crates/shared`: cross-cutting primitives
- `crates/domain`: pure-domain placeholder
- `crates/application`: use-case and ports placeholder
- `crates/infrastructure`: SQLx pool and migrations
- `crates/blockchain`: blockchain boundary placeholder
- `migrations`: SQLx SQL migrations

## Local Setup

1. Copy `.env.example` to `.env`.
2. Start Postgres: `docker compose up -d postgres`
3. Install SQLx CLI if needed:
   `cargo install sqlx-cli --no-default-features --features postgres`
4. Run migrations: `sqlx migrate run`
5. Start the API: `cargo run -p api`

## Useful Commands

- `cargo fmt --all`
- `cargo check --workspace`
- `cargo test --workspace`
- `make test-unit`
- `make test-integration`
- `make seed`
- `TEST_DATABASE_URL=postgres://... cargo test -p infrastructure --test repositories`
- `TEST_DATABASE_URL=postgres://... cargo test -p api --test api_flows -- --test-threads=1`
- `./scripts/test-ci.sh`
- `sqlx migrate run`
- `sqlx migrate info`

See also:
- `docs/testing.md`
- `docs/observability.md`

## Testing Strategy

- `domain` unit tests: pure rule validation and transition invariants.
- `application` unit tests: service/use-case behavior with fakes and mocks for external boundaries.
- `infrastructure` repository tests: real SQL against a migrated Postgres schema.
- `apps/api` integration tests: end-to-end HTTP flows against a real database-backed router.

Coverage priority is business critical first:
- auth flows: register/login
- savings and contribution flows: create plan, add milestone, add contributor, record/confirm contribution
- payouts: request and approve
- scholarships: application and decision

Database-backed tests are isolated and repeatable by:
- serializing access with a global test lock
- running migrations before tests
- truncating all tables between tests
- requiring `TEST_DATABASE_URL` or `DATABASE_URL` explicitly for DB-backed suites

## Files Created And Why

- Root workspace files: define build boundaries, formatting, linting, local DB, and repeatable commands
- `apps/api/*`: production-grade Axum entrypoint with graceful shutdown and explicit HTTP-only responsibilities
- `crates/infrastructure/*`: shared DB setup and migration execution so readiness is tied to real infrastructure
- `crates/shared/*`: minimal reusable primitives for IDs, money, currency, time, pagination, and client-safe error codes
- `migrations/*`: first durable schema needed before auth and audit work can start

## Next Step

The next correct slice is child profiles, plans, vaults, and milestones on top of the auth boundary that now exists.
