# Security

## Hardening Pass Summary

This repository is an API-first backend with financial workflows. The standard is not "good enough for CRUD". The standard is resistance to duplicate money movement, privilege escalation, token confusion, and sensitive operational data leakage.

Implemented in this pass:

- auth input normalization and stronger validation
- public registration blocked for privileged roles
- access JWTs now carry and validate an explicit `token_type`
- inactive users are denied during login, refresh, current-user lookup, and email verification
- JWT secret validation at boot:
  - minimum 32 characters
  - access and refresh secrets must differ
  - non-local environments must use JSON logs
- auth rate limiting fixed and tightened:
  - path bug corrected from `/auth` to `/api/v1/auth/*`
  - only sensitive auth endpoints are throttled
  - returns `429 Too Many Requests` with `Retry-After`
- secure response headers added:
  - `X-Content-Type-Options: nosniff`
  - `X-Frame-Options: DENY`
  - `Referrer-Policy: no-referrer`
  - `Permissions-Policy`
  - API-only `Content-Security-Policy`
  - `Cache-Control: no-store`
  - `Pragma: no-cache`
  - HSTS in staging/production
- financial replay protection improved:
  - contribution creation already idempotent
  - payout request creation now requires and persists `idempotency_key`
  - scholarship pool funding now requires `idempotency_key`
- DTO leakage reduced:
  - contribution external references no longer returned directly
  - payout external references no longer returned directly
  - payout review notes no longer returned directly
  - scholarship decision notes no longer returned directly
  - donor contribution external references no longer returned directly
- email verification token exposure reduced:
  - token only returned in `local`
  - non-local environments return metadata without the raw token
- audit log integrity improved:
  - audit records capture correlation/request IDs
  - audit metadata is sanitized before persistence/logging
  - audit logs remain queryable and logically separated from app logs

## Review Notes By Area

### Auth and Session Handling

What is hardened:
- email normalization is enforced at the application layer
- privileged self-registration is blocked for `platform_admin` and `school_admin`
- inactive users are denied in core auth flows
- access tokens are distinguished from refresh/email-verification tokens with explicit token type validation
- refresh tokens are hashed at rest
- refresh rotation still revokes the prior session before issuing a new one

What remains risky:
- access tokens are still self-contained bearer tokens and are not checked against live session state on every request
- revoked refresh sessions do not immediately invalidate already-issued access tokens
- MFA is still scaffold-only and not enforced for privileged actions

### Role Authorization

What is hardened:
- role checks remain explicit in service-layer rules
- public signup cannot mint privileged users

What remains risky:
- authorization is still mostly route/service local rather than centralized policy evaluation
- there is no secondary approval or step-up auth for high-risk admin actions

### DTO Validation

What is hardened:
- tighter length limits on auth tokens and sensitive free-text fields
- tighter idempotency key validation
- school payout references and operational references now have bounded length

What remains risky:
- validation is still manually duplicated across DTOs
- no request body size limiter is enforced at middleware level yet

### Rate Limiting

What is hardened:
- auth throttling now matches real API paths
- login/register/refresh/verification endpoints are rate-limited

What remains risky:
- the limiter is in-memory only
- it is not distributed and will not protect consistently across multiple instances
- `X-Forwarded-For` is used opportunistically and assumes trusted proxy behavior

### Secure Headers

What is hardened:
- API responses now ship restrictive browser-facing headers and no-store caching headers

What remains risky:
- this does not replace correct frontend escaping if a frontend later renders user-controlled values

### Secret Handling

What is hardened:
- JWT secrets are boot-validated for minimum strength and separation
- raw secrets are not logged or returned in API responses

What remains risky:
- there is no managed secret rotation workflow in-repo
- there is no KMS/HSM-backed signing path for JWTs yet

### SQL Injection

What is hardened:
- repository access uses parameterized SQLx bind parameters
- no raw user input string interpolation is used in the main persistence layer

Residual note:
- dynamic SQL should stay constrained to safe builder patterns only

### XSS

What is hardened:
- this API returns JSON, not server-rendered HTML
- sensitive user-entered notes/references are no longer reflected directly in several DTOs
- restrictive CSP and frame headers reduce API misuse in browser contexts

What remains risky:
- if a frontend later renders `notes`, `title`, or other user-controlled fields without escaping, XSS becomes a frontend problem immediately

### CSRF

Current posture:
- this API uses bearer tokens in headers/body, not cookie-authenticated browser sessions
- CSRF exposure is therefore materially lower than a cookie-based app

What remains risky:
- if the frontend later moves refresh/access handling into cookies, CSRF protection must be added immediately
- do not assume current posture survives future product changes

### Idempotency and Replay Protection

What is hardened:
- contributions require idempotency
- payout requests now require idempotency
- scholarship funding now requires idempotency
- some review transitions are naturally state-idempotent because invalid transitions are rejected

What remains risky:
- scholarship funding still relies on DB uniqueness/workflow behavior rather than a richer "return prior result on same key" pattern
- replay protection for admin review actions is state-based, not nonce-based

### Blockchain Signing Boundaries

What is hardened:
- signing boundaries are modeled explicitly in the blockchain abstraction
- audit logs avoid storing raw blockchain reference values where previews suffice

What remains risky:
- there is still no hardened key management or custody enforcement in this repo
- there is no cryptographic attestation that the caller is authorized to use a given signing boundary
- pre-signed envelope handling is still a sensitive surface and should be treated as high risk until contract-facing flows are fully verified

### Audit Trail Integrity

What is hardened:
- request and correlation IDs are attached to audit records
- audit logs are queryable and isolated from normal app log streams
- audit metadata is sanitized for obvious sensitive keys

What remains risky:
- audit records are stored in the same primary database trust domain as application data
- there is no append-only/WORM storage, hash chaining, or external tamper-evident sink

## Prioritized Remaining Risks

### 1. High: Access tokens remain valid after session revocation

Impact:
- stolen access tokens remain usable until expiry even after logout/refresh revocation

Required fix:
- add live session validation or a revocation/version check in authenticated request extraction

### 2. High: No enforced MFA or step-up authentication for privileged actions

Impact:
- payout review, school verification, and audit inspection rely on single-factor bearer auth

Required fix:
- enforce MFA for `platform_admin` and other privileged roles

### 3. High: In-memory rate limiting is not production-grade

Impact:
- attackers can bypass limits across instances or after restarts

Required fix:
- move rate limiting to shared infrastructure such as Redis or edge rate limiting

### 4. High: Blockchain custody/signing authorization is still under-specified

Impact:
- misuse of signing boundaries could create unauthorized on-chain actions

Required fix:
- bind signing boundary selection to authenticated actor authorization and managed key policy

### 5. Medium-High: Audit logs are not tamper-evident

Impact:
- an attacker with DB write access can alter both app data and its audit trail

Required fix:
- replicate audit logs to append-only external storage or add integrity chaining

### 6. Medium: Some financial replay protections are still uneven

Impact:
- not every money-adjacent workflow yet returns a stable prior result for duplicate keys

Required fix:
- standardize idempotency behavior across all money-moving and approval endpoints

### 7. Medium: No middleware-level request size limits

Impact:
- large payload abuse can still pressure memory and parsing paths

Required fix:
- add body size limits and fail early
