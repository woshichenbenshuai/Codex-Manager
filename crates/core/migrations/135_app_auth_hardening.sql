ALTER TABLE app_login_challenges ADD COLUMN setup_secret_ciphertext TEXT;
ALTER TABLE app_login_challenges ADD COLUMN initiated_by_user_id TEXT REFERENCES app_users(id) ON DELETE CASCADE;
ALTER TABLE app_login_challenges ADD COLUMN target_user_id TEXT REFERENCES app_users(id) ON DELETE CASCADE;
ALTER TABLE app_login_challenges ADD COLUMN locked_until INTEGER;

UPDATE app_login_challenges
SET target_user_id = user_id
WHERE target_user_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_app_login_challenges_target_purpose
    ON app_login_challenges(target_user_id, purpose, expires_at, used_at);

CREATE TABLE IF NOT EXISTS app_auth_throttle_buckets (
    scope TEXT NOT NULL,
    subject_hash TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    window_started_at INTEGER NOT NULL,
    locked_until INTEGER,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(scope, subject_hash)
);

CREATE INDEX IF NOT EXISTS idx_app_auth_throttle_cleanup
    ON app_auth_throttle_buckets(updated_at, locked_until);
