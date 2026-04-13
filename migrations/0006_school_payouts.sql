ALTER TABLE savings_vaults
ADD COLUMN IF NOT EXISTS total_disbursed_minor BIGINT NOT NULL DEFAULT 0 CHECK (total_disbursed_minor >= 0);

ALTER TABLE schools
ADD COLUMN IF NOT EXISTS legal_name TEXT,
ADD COLUMN IF NOT EXISTS display_name TEXT,
ADD COLUMN IF NOT EXISTS country TEXT,
ADD COLUMN IF NOT EXISTS payout_method TEXT,
ADD COLUMN IF NOT EXISTS payout_reference TEXT,
ADD COLUMN IF NOT EXISTS verification_status TEXT NOT NULL DEFAULT 'pending',
ADD COLUMN IF NOT EXISTS verified_by UUID NULL REFERENCES users(id),
ADD COLUMN IF NOT EXISTS verified_at TIMESTAMPTZ NULL;

UPDATE schools
SET legal_name = COALESCE(legal_name, name),
    display_name = COALESCE(display_name, name),
    country = COALESCE(country, country_code),
    payout_method = COALESCE(payout_method, 'manual'),
    payout_reference = COALESCE(payout_reference, payout_destination),
    verification_status = CASE
        WHEN verification_status IS NOT NULL THEN verification_status
        WHEN verified THEN 'verified'
        ELSE 'pending'
    END
WHERE legal_name IS NULL
   OR display_name IS NULL
   OR country IS NULL
   OR payout_method IS NULL
   OR payout_reference IS NULL;

ALTER TABLE schools
ALTER COLUMN legal_name SET NOT NULL,
ALTER COLUMN display_name SET NOT NULL,
ALTER COLUMN country SET NOT NULL,
ALTER COLUMN payout_method SET NOT NULL,
ALTER COLUMN payout_reference SET NOT NULL;

ALTER TABLE schools
DROP COLUMN IF EXISTS name,
DROP COLUMN IF EXISTS country_code,
DROP COLUMN IF EXISTS payout_destination,
DROP COLUMN IF EXISTS verified;

ALTER TABLE payout_requests
ADD COLUMN IF NOT EXISTS reviewed_by UUID NULL REFERENCES users(id),
ADD COLUMN IF NOT EXISTS reviewed_at TIMESTAMPTZ NULL;

CREATE INDEX IF NOT EXISTS idx_schools_verification_status ON schools (verification_status);
CREATE INDEX IF NOT EXISTS idx_schools_display_name ON schools (display_name);
CREATE INDEX IF NOT EXISTS idx_payout_requests_reviewed_by ON payout_requests (reviewed_by);
