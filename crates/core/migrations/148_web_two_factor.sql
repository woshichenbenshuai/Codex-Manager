CREATE TABLE IF NOT EXISTS web_two_factor_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    version BIGINT NOT NULL DEFAULT 0,
    state_json TEXT NOT NULL DEFAULT '{}'
);

INSERT OR IGNORE INTO web_two_factor_state (id, version, state_json) VALUES (1, 0, '{}');
