BEGIN;

INSERT INTO users (id, email, password_hash, role, email_verified, mfa_enabled, status)
VALUES
    ('10000000-0000-0000-0000-000000000001', 'parent.local@eduvault.dev', 'dev-password-hash', 'parent', TRUE, FALSE, 'active'),
    ('10000000-0000-0000-0000-000000000002', 'contributor.local@eduvault.dev', 'dev-password-hash', 'contributor', TRUE, FALSE, 'active'),
    ('10000000-0000-0000-0000-000000000003', 'admin.local@eduvault.dev', 'dev-password-hash', 'platform_admin', TRUE, FALSE, 'active'),
    ('10000000-0000-0000-0000-000000000004', 'donor.local@eduvault.dev', 'dev-password-hash', 'donor', TRUE, FALSE, 'active')
ON CONFLICT (email) DO NOTHING;

INSERT INTO child_profiles (id, owner_user_id, full_name, date_of_birth, education_level)
VALUES
    ('20000000-0000-0000-0000-000000000001', '10000000-0000-0000-0000-000000000001', 'Ada Local', DATE '2012-09-01', 'secondary')
ON CONFLICT (id) DO NOTHING;

INSERT INTO savings_plans (
    id, child_profile_id, owner_user_id, name, description, target_amount_minor, target_currency, status
)
VALUES
    (
        '30000000-0000-0000-0000-000000000001',
        '20000000-0000-0000-0000-000000000001',
        '10000000-0000-0000-0000-000000000001',
        'Ada 2026 Tuition Plan',
        'Local seed plan for manual walkthroughs',
        250000,
        'USD',
        'draft'
    )
ON CONFLICT (id) DO NOTHING;

INSERT INTO savings_vaults (
    id, plan_id, owner_user_id, currency, status, total_contributed_minor, total_locked_minor, total_disbursed_minor, version
)
VALUES
    (
        '40000000-0000-0000-0000-000000000001',
        '30000000-0000-0000-0000-000000000001',
        '10000000-0000-0000-0000-000000000001',
        'USD',
        'active',
        50000,
        0,
        0,
        0
    )
ON CONFLICT (id) DO NOTHING;

INSERT INTO vault_contributors (id, vault_id, contributor_user_id, role_label)
VALUES
    (
        '50000000-0000-0000-0000-000000000001',
        '40000000-0000-0000-0000-000000000001',
        '10000000-0000-0000-0000-000000000002',
        'family_friend'
    )
ON CONFLICT (vault_id, contributor_user_id) DO NOTHING;

INSERT INTO milestones (
    id, vault_id, title, description, due_date, target_amount_minor, funded_amount_minor, currency, payout_type, status
)
VALUES
    (
        '60000000-0000-0000-0000-000000000001',
        '40000000-0000-0000-0000-000000000001',
        'Term 1 Tuition',
        'Local seed milestone',
        CURRENT_DATE + INTERVAL '30 days',
        75000,
        0,
        'USD',
        'tuition',
        'planned'
    )
ON CONFLICT (id) DO NOTHING;

INSERT INTO schools (
    id, legal_name, display_name, country, payout_method, payout_reference, verification_status, verified_by, verified_at
)
VALUES
    (
        '70000000-0000-0000-0000-000000000001',
        'Springfield High School Ltd',
        'Springfield High',
        'NG',
        'manual',
        'school-acct-001',
        'verified',
        '10000000-0000-0000-0000-000000000003',
        NOW()
    )
ON CONFLICT (id) DO NOTHING;

INSERT INTO scholarship_pools (
    id, owner_user_id, name, description, status, available_funds_minor, currency,
    geography_restriction, education_level_restriction, school_id_restriction, category_restriction
)
VALUES
    (
        '80000000-0000-0000-0000-000000000001',
        '10000000-0000-0000-0000-000000000004',
        'Girls in STEM',
        'Seed pool for local demos',
        'open',
        150000,
        'USD',
        'NG',
        'secondary',
        NULL,
        'stem'
    )
ON CONFLICT (id) DO NOTHING;

INSERT INTO donor_contributions (
    id, pool_id, donor_user_id, amount_minor, currency, status, external_reference, idempotency_key
)
VALUES
    (
        '90000000-0000-0000-0000-000000000001',
        '80000000-0000-0000-0000-000000000001',
        '10000000-0000-0000-0000-000000000004',
        150000,
        'USD',
        'confirmed',
        'seed-donor-ref',
        'seed-donor-contribution-1'
    )
ON CONFLICT (idempotency_key) DO NOTHING;

COMMIT;
