ALTER TABLE payout_requests
ADD COLUMN IF NOT EXISTS idempotency_key TEXT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_payout_requests_idempotency_key
ON payout_requests (idempotency_key)
WHERE idempotency_key IS NOT NULL;
