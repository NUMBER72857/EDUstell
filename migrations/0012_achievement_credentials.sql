CREATE TABLE IF NOT EXISTS achievement_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    credential_ref UUID NOT NULL UNIQUE,
    child_profile_id UUID NOT NULL REFERENCES child_profiles(id) ON DELETE CASCADE,
    recipient_user_id UUID NULL REFERENCES users(id),
    school_id UUID NULL REFERENCES schools(id),
    achievement_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'issued',
    title TEXT NOT NULL,
    description TEXT NULL,
    achievement_date DATE NOT NULL,
    issued_by_user_id UUID NOT NULL REFERENCES users(id),
    issued_by_role TEXT NOT NULL,
    issuance_notes TEXT NULL,
    evidence_uri TEXT NULL,
    attestation_hash TEXT NOT NULL,
    attestation_method TEXT NOT NULL DEFAULT 'sha256',
    attestation_anchor TEXT NULL,
    attestation_anchor_network TEXT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT achievement_credentials_attestation_hash_len
        CHECK (char_length(attestation_hash) >= 32),
    CONSTRAINT achievement_credentials_anchor_requires_network
        CHECK (
            attestation_anchor IS NULL
            OR (attestation_anchor_network IS NOT NULL AND char_length(attestation_anchor_network) > 0)
        )
);

CREATE TRIGGER trg_achievement_credentials_updated_at
BEFORE UPDATE ON achievement_credentials
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE INDEX IF NOT EXISTS idx_achievement_credentials_child_profile_id
ON achievement_credentials (child_profile_id, achievement_date DESC, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_achievement_credentials_recipient_user_id
ON achievement_credentials (recipient_user_id, achievement_date DESC, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_achievement_credentials_issued_by_user_id
ON achievement_credentials (issued_by_user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_achievement_credentials_school_id
ON achievement_credentials (school_id, created_at DESC);
