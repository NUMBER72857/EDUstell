ALTER TABLE notifications
    ADD COLUMN IF NOT EXISTS notification_type TEXT,
    ADD COLUMN IF NOT EXISTS metadata JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE notifications
SET notification_type = CASE
    WHEN lower(title) LIKE '%contribution%' THEN 'contribution_received'
    WHEN lower(title) LIKE '%due soon%' THEN 'milestone_due_soon'
    WHEN lower(title) LIKE '%underfunded%' THEN 'milestone_underfunded'
    WHEN lower(title) LIKE '%approved%' THEN 'payout_approved'
    WHEN lower(title) LIKE '%completed%' THEN 'payout_completed'
    WHEN lower(title) LIKE '%scholarship%' THEN 'scholarship_awarded'
    ELSE 'kyc_action_required'
END
WHERE notification_type IS NULL;

ALTER TABLE notifications
    ALTER COLUMN notification_type SET NOT NULL;

CREATE TABLE IF NOT EXISTS notification_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type TEXT NOT NULL,
    in_app_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    email_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT notification_preferences_user_type_unique UNIQUE (user_id, notification_type)
);

CREATE TRIGGER trg_notification_preferences_updated_at
BEFORE UPDATE ON notification_preferences
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE INDEX IF NOT EXISTS idx_notifications_user_created_at
ON notifications (user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_notifications_user_type
ON notifications (user_id, notification_type);

CREATE INDEX IF NOT EXISTS idx_notification_preferences_user_id
ON notification_preferences (user_id);
