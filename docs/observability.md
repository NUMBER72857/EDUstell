# Observability Notes

## What is instrumented

- Structured application logs through `tracing`
- Dedicated audit event stream using the `audit` tracing target
- Request and correlation IDs on every request via `x-request-id` and `x-correlation-id`
- Queryable `audit_logs` records with indexed `request_id` and `correlation_id`
- Liveness and readiness endpoints:
  - `GET /api/v1/health`
  - `GET /api/v1/health/live`
  - `GET /api/v1/ready`
  - `GET /api/v1/health/ready`
- Internal metrics endpoint:
  - `GET /api/v1/internal/metrics`
- Admin audit inspection endpoint:
  - `GET /api/v1/admin/audit-logs`

## Sensitive action coverage

- `savings_plan.created`
- `milestone.created`
- `vault_contributor.added`
- `contribution.recorded`
- `payout.requested`
- `payout.approved`
- `payout.rejected`
- `scholarship_application.approved`
- `scholarship_application.rejected`
- `school.verification_status_changed`

## Audit query examples

- Filter by correlation:
  - `/api/v1/admin/audit-logs?correlation_id=<id>`
- Filter by request:
  - `/api/v1/admin/audit-logs?request_id=<id>`
- Filter by entity:
  - `/api/v1/admin/audit-logs?entity_type=school&entity_id=<uuid>`
- Filter by action:
  - `/api/v1/admin/audit-logs?action=payout.approved`

## Logging discipline

- Audit events are emitted to the `audit` target and formatted separately from normal application logs.
- Request logging does not include request bodies or authorization headers.
- Audit metadata is sanitized before persistence and log emission.
- Known sensitive fields such as secrets, tokens, passwords, payout references, and review notes are redacted.

## Basic dashboard ideas

- Request volume:
  - `total_requests`
  - `in_flight_requests`
- Reliability:
  - `error_responses`
  - readiness failures from probes
- Performance:
  - `average_latency_ms`
- Governance:
  - audit event count by `action`
  - audit events grouped by `correlation_id`
  - release approvals vs rejections
  - school verification decisions
