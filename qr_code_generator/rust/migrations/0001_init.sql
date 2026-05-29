-- url_mappings: one row per short link.
-- token is UNIQUE so the DB itself enforces collision-freedom; the application
-- relies on the unique-constraint violation to drive its retry loop.
CREATE TABLE IF NOT EXISTS url_mappings (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    token        TEXT    NOT NULL UNIQUE,
    original_url TEXT    NOT NULL,
    created_at   TEXT    NOT NULL,            -- RFC3339, written/read via chrono
    updated_at   TEXT    NOT NULL,
    expires_at   TEXT,                        -- nullable; NULL = never expires
    is_deleted   INTEGER NOT NULL DEFAULT 0   -- 0/1 boolean (soft delete)
);
CREATE INDEX IF NOT EXISTS idx_mappings_token ON url_mappings(token);

-- scan_events: append-only log, one row per redirect served.
CREATE TABLE IF NOT EXISTS scan_events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    token      TEXT    NOT NULL,
    scanned_at TEXT    NOT NULL,
    user_agent TEXT,
    ip_address TEXT
);
CREATE INDEX IF NOT EXISTS idx_scan_token_time ON scan_events(token, scanned_at);
