ALTER TABLE scholarship_pools
    ADD COLUMN IF NOT EXISTS geography_restriction TEXT NULL,
    ADD COLUMN IF NOT EXISTS education_level_restriction TEXT NULL,
    ADD COLUMN IF NOT EXISTS school_id_restriction UUID NULL REFERENCES schools(id),
    ADD COLUMN IF NOT EXISTS category_restriction TEXT NULL;

ALTER TABLE scholarship_applications
    ADD COLUMN IF NOT EXISTS student_country TEXT NULL,
    ADD COLUMN IF NOT EXISTS education_level TEXT NULL,
    ADD COLUMN IF NOT EXISTS school_id UUID NULL REFERENCES schools(id),
    ADD COLUMN IF NOT EXISTS category TEXT NULL;

ALTER TABLE scholarship_awards
    ALTER COLUMN amount_minor DROP NOT NULL;

ALTER TABLE scholarship_awards
    DROP CONSTRAINT IF EXISTS scholarship_awards_amount_minor_check;

ALTER TABLE scholarship_awards
    ADD COLUMN IF NOT EXISTS decided_by UUID NULL REFERENCES users(id),
    ADD COLUMN IF NOT EXISTS decision_notes TEXT NULL,
    ADD COLUMN IF NOT EXISTS linked_payout_request_id UUID NULL REFERENCES payout_requests(id),
    ADD COLUMN IF NOT EXISTS linked_vault_id UUID NULL REFERENCES savings_vaults(id);

UPDATE scholarship_awards
SET amount_minor = 0
WHERE amount_minor IS NULL;

ALTER TABLE scholarship_awards
    ALTER COLUMN amount_minor SET NOT NULL;

ALTER TABLE scholarship_awards
    ADD CONSTRAINT scholarship_awards_amount_minor_check CHECK (amount_minor >= 0);

CREATE TABLE IF NOT EXISTS donor_contributions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id UUID NOT NULL REFERENCES scholarship_pools(id) ON DELETE CASCADE,
    donor_user_id UUID NOT NULL REFERENCES users(id),
    amount_minor BIGINT NOT NULL CHECK (amount_minor > 0),
    currency TEXT NOT NULL,
    status TEXT NOT NULL,
    external_reference TEXT NULL,
    idempotency_key TEXT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER trg_donor_contributions_updated_at
BEFORE UPDATE ON donor_contributions
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE INDEX IF NOT EXISTS idx_donor_contributions_pool_id
ON donor_contributions (pool_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_donor_contributions_donor_user_id
ON donor_contributions (donor_user_id, created_at DESC);
