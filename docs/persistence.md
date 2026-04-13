# Persistence Design

## Schema Decisions

- UUIDs are used across every table for consistency and easier cross-boundary references.
- All tables include `created_at` and `updated_at`.
- `updated_at` is maintained by a shared PostgreSQL trigger so correctness does not rely on every update query remembering it.
- Money is stored as `BIGINT` minor units plus a separate currency column.
- `audit_logs` stores structured JSONB metadata for traceability.
- `savings_vaults.version` supports optimistic concurrency on balance updates.

## Locking Strategy

Financial mutation paths should use a transaction plus pessimistic row locks:

- lock the vault row with `SELECT ... FOR UPDATE`
- lock the payout row with `SELECT ... FOR UPDATE` when approving/rejecting

Why:

- balance mutations are hot rows
- double-approval or concurrent settlement is worse than brief contention
- PostgreSQL row locks are simpler and safer than trying to reconstruct correctness after races

Optimistic version checks are also used on vault balance writes so stale writers fail fast.

## Example Queries

Create contribution:

```sql
INSERT INTO contributions
    (id, vault_id, contributor_user_id, amount_minor, currency, status, source_type, external_reference, idempotency_key)
VALUES
    ($1, $2, $3, $4, $5, $6, $7, $8, $9);
```

Lock vault for settlement:

```sql
SELECT id, plan_id, owner_user_id, currency, status, total_contributed_minor, total_locked_minor, external_wallet_account_id, external_contract_ref, version, created_at, updated_at
FROM savings_vaults
WHERE id = $1
FOR UPDATE;
```

Optimistic balance update:

```sql
UPDATE savings_vaults
SET total_contributed_minor = $2,
    total_locked_minor = $3,
    version = version + 1
WHERE id = $1
  AND version = $4;
```

Append audit log:

```sql
INSERT INTO audit_logs
    (id, actor_user_id, entity_type, entity_id, action, metadata)
VALUES
    ($1, $2, $3, $4, $5, $6);
```
