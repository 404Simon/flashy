CREATE TABLE oauth_rate_limits (
    rate_key TEXT NOT NULL,
    window_start INTEGER NOT NULL,
    request_count INTEGER NOT NULL CHECK (request_count > 0),
    PRIMARY KEY (rate_key, window_start)
);

CREATE INDEX idx_oauth_rate_limits_window ON oauth_rate_limits(window_start);
