CREATE TABLE IF NOT EXISTS external_references (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    reference_kind TEXT NOT NULL,
    value TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_type, entity_id, reference_kind)
);

CREATE TABLE IF NOT EXISTS blockchain_transaction_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    operation_kind TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    tx_hash TEXT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error_code TEXT NULL,
    last_error_message TEXT NULL,
    next_retry_at TIMESTAMPTZ NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_type, entity_id, operation_kind)
);

CREATE TRIGGER trg_external_references_updated_at
BEFORE UPDATE ON external_references
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_blockchain_transaction_records_updated_at
BEFORE UPDATE ON blockchain_transaction_records
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE INDEX IF NOT EXISTS idx_external_references_entity
ON external_references (entity_type, entity_id);

CREATE INDEX IF NOT EXISTS idx_external_references_kind
ON external_references (reference_kind);

CREATE INDEX IF NOT EXISTS idx_blockchain_transaction_records_entity
ON blockchain_transaction_records (entity_type, entity_id, operation_kind);

CREATE INDEX IF NOT EXISTS idx_blockchain_transaction_records_status
ON blockchain_transaction_records (status, next_retry_at);
