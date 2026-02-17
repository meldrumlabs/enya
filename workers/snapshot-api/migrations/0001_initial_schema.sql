-- Track snapshot uploads per GitHub user

CREATE TABLE users (
    github_id INTEGER PRIMARY KEY,
    github_login TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE snapshots (
    id TEXT PRIMARY KEY,
    github_id INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (github_id) REFERENCES users(github_id)
);

CREATE INDEX idx_snapshots_github_id ON snapshots(github_id);
