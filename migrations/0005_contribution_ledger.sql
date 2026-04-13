CREATE TABLE IF NOT EXISTS vault_ledger_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vault_id UUID NOT NULL REFERENCES savings_vaults(id) ON DELETE CASCADE,
    contribution_id UUID NULL REFERENCES contributions(id) ON DELETE SET NULL,
    actor_user_id UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    entry_type TEXT NOT NULL,
    amount_minor BIGINT NOT NULL,
    currency TEXT NOT NULL,
    balance_after_minor BIGINT NOT NULL CHECK (balance_after_minor >= 0),
    external_reference TEXT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER trg_vault_ledger_entries_updated_at
BEFORE UPDATE ON vault_ledger_entries
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE INDEX IF NOT EXISTS idx_vault_ledger_entries_vault_id
ON vault_ledger_entries (vault_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_vault_ledger_entries_contribution_id
ON vault_ledger_entries (contribution_id);

CREATE INDEX IF NOT EXISTS idx_vault_ledger_entries_entry_type
ON vault_ledger_entries (entry_type);
