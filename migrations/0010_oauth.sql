CREATE TABLE oauth_clients (
    client_id TEXT PRIMARY KEY,
    registration_source TEXT NOT NULL CHECK (registration_source IN ('dcr', 'cimd', 'configured')),
    display_name TEXT NOT NULL,
    redirect_uris TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER
);

CREATE TABLE oauth_authorization_requests (
    request_id_hash BLOB PRIMARY KEY,
    browser_binding_hash BLOB NOT NULL,
    client_id TEXT NOT NULL REFERENCES oauth_clients(client_id) ON DELETE CASCADE,
    redirect_uri TEXT NOT NULL,
    resource TEXT NOT NULL,
    scopes TEXT NOT NULL,
    state TEXT,
    code_challenge TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    consumed_at INTEGER
);

CREATE TABLE oauth_grants (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL REFERENCES oauth_clients(client_id) ON DELETE CASCADE,
    resource TEXT NOT NULL,
    scopes TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    absolute_expires_at INTEGER NOT NULL,
    last_refresh_at INTEGER NOT NULL,
    revoked_at INTEGER
);

CREATE TABLE oauth_authorization_codes (
    code_hash BLOB PRIMARY KEY,
    grant_id TEXT NOT NULL REFERENCES oauth_grants(id) ON DELETE CASCADE,
    redirect_uri TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    resource TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    consumed_at INTEGER
);

CREATE TABLE oauth_access_tokens (
    token_hash BLOB PRIMARY KEY,
    grant_id TEXT NOT NULL REFERENCES oauth_grants(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL
);

CREATE TABLE oauth_refresh_tokens (
    token_hash BLOB PRIMARY KEY,
    grant_id TEXT NOT NULL REFERENCES oauth_grants(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL,
    consumed_at INTEGER,
    successor_hash BLOB
);

CREATE INDEX idx_oauth_requests_expiry ON oauth_authorization_requests(expires_at);
CREATE INDEX idx_oauth_grants_user_client ON oauth_grants(user_id, client_id);
CREATE INDEX idx_oauth_grants_expiry ON oauth_grants(absolute_expires_at);
CREATE INDEX idx_oauth_codes_grant ON oauth_authorization_codes(grant_id);
CREATE INDEX idx_oauth_codes_expiry ON oauth_authorization_codes(expires_at);
CREATE INDEX idx_oauth_access_grant ON oauth_access_tokens(grant_id);
CREATE INDEX idx_oauth_access_expiry ON oauth_access_tokens(expires_at);
CREATE INDEX idx_oauth_refresh_grant ON oauth_refresh_tokens(grant_id);
CREATE INDEX idx_oauth_refresh_expiry ON oauth_refresh_tokens(expires_at);
