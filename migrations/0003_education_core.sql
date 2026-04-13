CREATE TABLE IF NOT EXISTS child_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id UUID NOT NULL REFERENCES users(id),
    full_name TEXT NOT NULL,
    date_of_birth DATE NULL,
    education_level TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS wallet_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    network TEXT NOT NULL,
    address TEXT NOT NULL,
    label TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (network, address)
);

CREATE TABLE IF NOT EXISTS savings_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    child_profile_id UUID NOT NULL REFERENCES child_profiles(id),
    owner_user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    description TEXT NULL,
    target_amount_minor BIGINT NOT NULL CHECK (target_amount_minor >= 0),
    target_currency TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS savings_vaults (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id UUID NOT NULL REFERENCES savings_plans(id),
    owner_user_id UUID NOT NULL REFERENCES users(id),
    currency TEXT NOT NULL,
    status TEXT NOT NULL,
    total_contributed_minor BIGINT NOT NULL DEFAULT 0 CHECK (total_contributed_minor >= 0),
    total_locked_minor BIGINT NOT NULL DEFAULT 0 CHECK (total_locked_minor >= 0),
    external_wallet_account_id UUID NULL REFERENCES wallet_accounts(id),
    external_contract_ref TEXT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS vault_contributors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vault_id UUID NOT NULL REFERENCES savings_vaults(id) ON DELETE CASCADE,
    contributor_user_id UUID NOT NULL REFERENCES users(id),
    role_label TEXT NOT NULL DEFAULT 'contributor',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (vault_id, contributor_user_id)
);

CREATE TABLE IF NOT EXISTS milestones (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vault_id UUID NOT NULL REFERENCES savings_vaults(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NULL,
    due_date DATE NOT NULL,
    target_amount_minor BIGINT NOT NULL CHECK (target_amount_minor > 0),
    funded_amount_minor BIGINT NOT NULL DEFAULT 0 CHECK (funded_amount_minor >= 0),
    currency TEXT NOT NULL,
    payout_type TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS contributions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vault_id UUID NOT NULL REFERENCES savings_vaults(id) ON DELETE CASCADE,
    contributor_user_id UUID NOT NULL REFERENCES users(id),
    amount_minor BIGINT NOT NULL CHECK (amount_minor > 0),
    currency TEXT NOT NULL,
    status TEXT NOT NULL,
    source_type TEXT NOT NULL,
    external_reference TEXT NULL,
    idempotency_key TEXT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS schools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    country_code TEXT NOT NULL,
    payout_destination TEXT NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS payout_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vault_id UUID NOT NULL REFERENCES savings_vaults(id),
    milestone_id UUID NOT NULL REFERENCES milestones(id),
    school_id UUID NOT NULL REFERENCES schools(id),
    requested_by UUID NOT NULL REFERENCES users(id),
    amount_minor BIGINT NOT NULL CHECK (amount_minor > 0),
    currency TEXT NOT NULL,
    status TEXT NOT NULL,
    review_notes TEXT NULL,
    external_payout_reference TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER trg_child_profiles_updated_at BEFORE UPDATE ON child_profiles FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_wallet_accounts_updated_at BEFORE UPDATE ON wallet_accounts FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_savings_plans_updated_at BEFORE UPDATE ON savings_plans FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_savings_vaults_updated_at BEFORE UPDATE ON savings_vaults FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_vault_contributors_updated_at BEFORE UPDATE ON vault_contributors FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_milestones_updated_at BEFORE UPDATE ON milestones FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_contributions_updated_at BEFORE UPDATE ON contributions FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_schools_updated_at BEFORE UPDATE ON schools FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_payout_requests_updated_at BEFORE UPDATE ON payout_requests FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE INDEX IF NOT EXISTS idx_child_profiles_owner_user_id ON child_profiles (owner_user_id);
CREATE INDEX IF NOT EXISTS idx_wallet_accounts_user_id ON wallet_accounts (user_id);
CREATE INDEX IF NOT EXISTS idx_savings_plans_child_profile_id ON savings_plans (child_profile_id);
CREATE INDEX IF NOT EXISTS idx_savings_plans_owner_user_id ON savings_plans (owner_user_id);
CREATE INDEX IF NOT EXISTS idx_savings_plans_status ON savings_plans (status);
CREATE INDEX IF NOT EXISTS idx_savings_vaults_plan_id ON savings_vaults (plan_id);
CREATE INDEX IF NOT EXISTS idx_savings_vaults_owner_user_id ON savings_vaults (owner_user_id);
CREATE INDEX IF NOT EXISTS idx_vault_contributors_vault_id ON vault_contributors (vault_id);
CREATE INDEX IF NOT EXISTS idx_vault_contributors_contributor_user_id ON vault_contributors (contributor_user_id);
CREATE INDEX IF NOT EXISTS idx_milestones_vault_id ON milestones (vault_id);
CREATE INDEX IF NOT EXISTS idx_milestones_due_date ON milestones (due_date);
CREATE INDEX IF NOT EXISTS idx_milestones_status ON milestones (status);
CREATE INDEX IF NOT EXISTS idx_contributions_vault_id ON contributions (vault_id);
CREATE INDEX IF NOT EXISTS idx_contributions_contributor_user_id ON contributions (contributor_user_id);
CREATE INDEX IF NOT EXISTS idx_contributions_status ON contributions (status);
CREATE INDEX IF NOT EXISTS idx_schools_verified ON schools (verified);
CREATE INDEX IF NOT EXISTS idx_payout_requests_vault_id ON payout_requests (vault_id);
CREATE INDEX IF NOT EXISTS idx_payout_requests_milestone_id ON payout_requests (milestone_id);
CREATE INDEX IF NOT EXISTS idx_payout_requests_school_id ON payout_requests (school_id);
CREATE INDEX IF NOT EXISTS idx_payout_requests_status ON payout_requests (status);
