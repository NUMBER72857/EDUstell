# Environment Variables

## Application

- `APP_NAME`
  - service name used in health responses and logs
  - example: `EDUstell`
- `APP_ENV`
  - allowed: `local`, `development`, `staging`, `production`
  - affects config validation and security header behavior
- `APP_HOST`
  - bind address for the API server
  - use `0.0.0.0` in containers
- `APP_PORT`
  - listen port for the API server

## Database

- `DATABASE_URL`
  - required by the API, migrations, and seed scripts
  - Postgres connection string
- `TEST_DATABASE_URL`
  - used by DB-backed tests
  - keep separate from production data

## Logging

- `RUST_LOG`
  - tracing filter
  - examples:
    - local: `info,api=debug,infrastructure=info`
    - production: `info`
- `LOG_FORMAT`
  - `pretty` for local development
  - `json` required for staging/production by app config validation

## JWT

- `JWT_ACCESS_SECRET`
  - required
  - minimum 32 characters
  - must differ from `JWT_REFRESH_SECRET`
- `JWT_REFRESH_SECRET`
  - required
  - minimum 32 characters
- `JWT_ACCESS_TTL_SECS`
  - recommended short TTL
  - default example: `900`
- `JWT_REFRESH_TTL_SECS`
  - longer-lived refresh session TTL
  - default example: `2592000`

## Local Compose Postgres

- `POSTGRES_DB`
- `POSTGRES_USER`
- `POSTGRES_PASSWORD`

These are for local or self-managed compose deployments only. They are not consumed by the Rust application directly unless you build `DATABASE_URL` from them.

## Dev vs Production

### Development assumptions

- `APP_ENV=local` or `development`
- `LOG_FORMAT=pretty`
- local Postgres is acceptable
- seed data may be loaded
- email verification token may be returned in local responses

### Production assumptions

- `APP_ENV=production`
- `LOG_FORMAT=json`
- secrets injected externally, never committed
- managed backups for Postgres
- no seed data loading
- use unique production-grade JWT secrets
- run migrations before new app version receives traffic

## Secret Handling Rules

- never commit `.env`
- never commit production secrets
- use your deployment platform secret store or OS-level secret injection
- rotate JWT secrets intentionally; do not reuse local/dev values in shared environments
