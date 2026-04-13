ALTER TABLE audit_logs
ADD COLUMN IF NOT EXISTS request_id TEXT NULL,
ADD COLUMN IF NOT EXISTS correlation_id TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_audit_logs_request_id ON audit_logs (request_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_correlation_id ON audit_logs (correlation_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action_entity_created_at
ON audit_logs (action, entity_type, created_at DESC);
