ALTER TABLE app_users ADD COLUMN totp_secret_ciphertext TEXT;
ALTER TABLE app_users ADD COLUMN totp_pending_secret_ciphertext TEXT;
ALTER TABLE app_users ADD COLUMN totp_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE app_users ADD COLUMN totp_confirmed_at INTEGER;
ALTER TABLE app_users ADD COLUMN totp_last_used_step INTEGER;

CREATE TABLE IF NOT EXISTS app_login_challenges (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    purpose TEXT NOT NULL DEFAULT 'login',
    expires_at INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at INTEGER,
    used_at INTEGER,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_app_login_challenges_user
    ON app_login_challenges(user_id, expires_at);
CREATE INDEX IF NOT EXISTS idx_app_login_challenges_token
    ON app_login_challenges(token_hash, expires_at);
