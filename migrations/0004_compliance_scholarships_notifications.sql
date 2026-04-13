CREATE TABLE IF NOT EXISTS kyc_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL UNIQUE REFERENCES users(id),
    status TEXT NOT NULL,
    provider_reference TEXT NULL,
    reviewed_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS scholarship_pools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    description TEXT NULL,
    status TEXT NOT NULL,
    available_funds_minor BIGINT NOT NULL CHECK (available_funds_minor >= 0),
    currency TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS scholarship_applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pool_id UUID NOT NULL REFERENCES scholarship_pools(id) ON DELETE CASCADE,
    applicant_user_id UUID NOT NULL REFERENCES users(id),
    child_profile_id UUID NOT NULL REFERENCES child_profiles(id),
    status TEXT NOT NULL,
    notes TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS scholarship_awards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES scholarship_applications(id) ON DELETE CASCADE,
    amount_minor BIGINT NOT NULL CHECK (amount_minor > 0),
    currency TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL,
    read_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER trg_kyc_profiles_updated_at BEFORE UPDATE ON kyc_profiles FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_scholarship_pools_updated_at BEFORE UPDATE ON scholarship_pools FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_scholarship_applications_updated_at BEFORE UPDATE ON scholarship_applications FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_scholarship_awards_updated_at BEFORE UPDATE ON scholarship_awards FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_notifications_updated_at BEFORE UPDATE ON notifications FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE INDEX IF NOT EXISTS idx_kyc_profiles_user_id ON kyc_profiles (user_id);
CREATE INDEX IF NOT EXISTS idx_kyc_profiles_status ON kyc_profiles (status);
CREATE INDEX IF NOT EXISTS idx_scholarship_pools_owner_user_id ON scholarship_pools (owner_user_id);
CREATE INDEX IF NOT EXISTS idx_scholarship_pools_status ON scholarship_pools (status);
CREATE INDEX IF NOT EXISTS idx_scholarship_applications_pool_id ON scholarship_applications (pool_id);
CREATE INDEX IF NOT EXISTS idx_scholarship_applications_applicant_user_id ON scholarship_applications (applicant_user_id);
CREATE INDEX IF NOT EXISTS idx_scholarship_applications_status ON scholarship_applications (status);
CREATE INDEX IF NOT EXISTS idx_scholarship_awards_application_id ON scholarship_awards (application_id);
CREATE INDEX IF NOT EXISTS idx_notifications_user_id ON notifications (user_id);
CREATE INDEX IF NOT EXISTS idx_notifications_status ON notifications (status);
