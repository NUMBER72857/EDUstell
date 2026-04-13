# Deployment Notes

## Scope

This repository currently contains a deployable Rust API and supporting database/migration/seed tooling.

It does **not** contain a deployable web application. `Dockerfile.web` is a deliberate scaffold that fails fast so nobody mistakes this repo for a full web+api monorepo.

## Infra Dependencies

Minimum required infrastructure:

- PostgreSQL 16+
- a process runner or container runtime for the API
- persistent secret injection for JWT secrets and database credentials
- centralized log collection for JSON logs in non-local environments

Optional but strongly recommended:

- reverse proxy / load balancer with TLS termination
- metrics scraping / alerting
- externalized rate limiting store
- append-only or replicated audit log sink

## Containers

### API image

Build:

```bash
docker build -f Dockerfile.api -t eduvault-api:latest .
```

Run:

```bash
docker run --rm -p 8080:8080 --env-file .env eduvault-api:latest
```

### Web image

`Dockerfile.web` is intentionally non-deployable until a real frontend exists in-repo.

## Compose

### Development

Use:

```bash
docker compose up -d postgres
```

This is local-only and assumes:
- local Postgres credentials
- local migration execution
- local app execution outside compose

### Production-style self-managed compose

Use:

```bash
docker compose -f docker-compose.prod.yml --env-file .env up -d postgres
docker compose -f docker-compose.prod.yml --env-file .env --profile ops run --rm migrate
docker compose -f docker-compose.prod.yml --env-file .env up -d api
```

Notes:
- the `seed` profile is for non-production bootstrap only
- do not run seed data in production
- `docker-compose.prod.yml` is provider-neutral and self-managed; it is not a managed cloud template

## Migrations

Run migrations before routing traffic to a new app version:

```bash
DATABASE_URL=postgres://... ./scripts/migrate.sh
```

Check migration state:

```bash
DATABASE_URL=postgres://... ./scripts/migrate-check.sh
```

Operational rule:
- schema-forward deploys only
- do not assume rollback safety unless migrations are explicitly reversible

## Seed Data

For local development only:

```bash
DATABASE_URL=postgres://... ./scripts/seed-local.sh
```

This loads:
- parent, contributor, donor, and admin users
- child profile
- savings plan and vault
- milestone
- verified school
- scholarship pool and donor contribution

## Health Checks

Available endpoints:

- liveness: `GET /api/v1/health`
- liveness alias: `GET /api/v1/health/live`
- readiness: `GET /api/v1/ready`
- readiness alias: `GET /api/v1/health/ready`

Interpretation:

- `health` means process is up
- `ready` means process is up and database connectivity works

Do not route external traffic based only on liveness.

## Readiness Guidance

Recommended rollout order:

1. bring up Postgres
2. run migrations
3. start API
4. wait for `/api/v1/health/ready` to return `200`
5. start sending traffic

Recommended shutdown behavior:

- stop sending new traffic first
- allow in-flight requests to drain
- then terminate the API process

## Logging Guidance

Local:
- `LOG_FORMAT=pretty`
- use verbose `RUST_LOG` as needed

Staging/production:
- `LOG_FORMAT=json`
- keep application logs and audit logs centralized
- audit events use the `audit` tracing target and should be indexed separately
- do not rely on container stdout retention alone

Retention guidance:
- retain audit logs longer than standard app logs
- ensure correlation IDs are searchable across request, app, and audit logs

## Production Config Notes

- use `APP_ENV=production`
- bind `APP_HOST=0.0.0.0`
- keep `APP_PORT` consistent with probe and proxy config
- use long, unique JWT secrets; never reuse development values
- set `RUST_LOG` conservatively to avoid noisy high-cardinality logs
- prefer managed Postgres backups and point-in-time recovery
- place the API behind TLS termination
- keep the API stateless; session durability lives in Postgres

## GitHub Actions CI

Workflow file:

- `.github/workflows/ci.yml`

It runs:
- checkout
- Rust toolchain install
- SQLx CLI install
- lint
- tests
- release build
- API Docker image build

The workflow explicitly notes that a web build is skipped because no web application exists in this repository.

## Unresolved Deployment Gaps

- no real web deployment artifact exists
- in-memory rate limiting is not production-grade for multi-instance deployments
- DB-backed readiness is present, but there is no startup backoff/orchestrator-specific retry policy here
- audit logs are not yet exported to a tamper-evident sink
