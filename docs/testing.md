# EduVault Testing Strategy

## Risk-Based Priorities

Focus on failure paths that can lose money, break approvals, or corrupt workflow state before chasing broad coverage.

Priority 1:
- contribution validation and settlement transitions
- payout request and review transitions
- scholarship pool funding and decision logic
- school verification
- auditability of sensitive actions

Priority 2:
- auth/session lifecycle
- notifications and preference behavior
- admin observability endpoints

Priority 3:
- frontend component rendering states
- Soroban contract boundary behavior once contracts exist in-repo

## Test Structure

### 1. Domain unit tests

Location:
- `crates/domain/src/*`

Purpose:
- validate pure business rules
- assert state transition invariants
- keep tests deterministic and fast

Examples in repo:
- contribution transition guards
- payout review and school verification rules
- scholarship restriction and award transition rules

### 2. Application service unit tests

Location:
- `crates/application/src/*`

Purpose:
- verify use-case orchestration with fake repositories
- assert audit creation and role checks
- avoid real database and network dependencies

Examples in repo:
- savings service audit tests
- contribution and payout workflow tests
- school create/verify audit tests

### 3. Infrastructure integration tests

Location:
- `crates/infrastructure/tests/*`

Purpose:
- validate SQLx mappings, migrations, constraints, and repository behavior
- run against a real Postgres schema
- keep scope narrow and DB-focused

Examples in repo:
- audit log repository query coverage
- notification read/unread lifecycle

### 4. API integration tests

Location:
- `apps/api/tests/*`

Purpose:
- exercise critical HTTP flows end to end against the router
- verify status codes, envelopes, and the most important workflow happy paths
- prefer a few durable high-signal flows over many fragile route-by-route tests

Examples in repo:
- register/login
- contribution -> payout review/approval
- scholarship application -> approval
- audit inspection for a sensitive contribution flow

## External Provider Policy

- Mock external providers in unit tests.
- Keep blockchain/Soroban provider tests at the boundary layer, not deep inside unrelated service tests.
- Do not let CI depend on external RPCs, email providers, or hosted auth services.

## Soroban Contract Test Plan

Current status:
- no Soroban contract workspace exists in this repo yet

When contracts land, add:
- unit tests for contract state transitions and access control
- contract invocation fixtures for `create_vault`, `contribute`, `lock_funds`, `release`
- host-based tests using Soroban SDK test utilities
- contract/API boundary tests that verify argument shape, idempotency keys, and error mapping

Suggested layout:
- `contracts/<contract-name>/src/lib.rs`
- `contracts/<contract-name>/src/test.rs`
- `contracts/fixtures/`

## Frontend Component Test Plan

Current status:
- no frontend app exists in this repo yet

When frontend lands, add component tests for:
- loading state
- empty state
- happy path populated state
- validation error state
- permission-denied state
- retry/error boundary state

Suggested targets first:
- savings plan wizard
- milestone form
- contribution checkout state machine
- payout review panel
- scholarship pool funding form

## E2E Outline

The following flows should be promoted to browser-driven E2E once UI and contract surfaces exist:

1. Create savings plan
   - parent logs in
   - chooses child profile
   - creates plan
   - verifies audit event and success state

2. Add milestone
   - parent opens vault
   - creates milestone
   - verifies milestone appears and audit event exists

3. Contribute
   - contributor or parent initiates contribution
   - system confirms contribution
   - vault balance and ledger update are visible

4. Request payout
   - parent chooses milestone and verified school
   - submits payout request
   - payout enters pending state

5. Review payout
   - platform admin moves payout to review
   - approves or rejects
   - audit log reflects decision

6. Fund scholarship pool
   - donor creates or opens pool
   - funds pool
   - available balance updates

## Local Seed Data

Use:
- `make seed`
- or `DATABASE_URL=postgres://... ./scripts/seed-local.sh`

Seed contents:
- parent, contributor, donor, and platform admin users
- one child profile
- one savings plan and vault
- one milestone
- one verified school
- one funded scholarship pool

## CI-Friendly Commands

- `./scripts/test-unit.sh`
- `./scripts/test-integration.sh`
- `./scripts/test-contracts.sh`
- `./scripts/test-frontend.sh`
- `./scripts/test-ci.sh`

The contract and frontend scripts intentionally no-op with a clear message until those codebases exist. That is deliberate. Fake green tests are worse than explicit gaps.
