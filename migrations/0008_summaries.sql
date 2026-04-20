CREATE TABLE summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    content_markdown TEXT NOT NULL,
    file_id INTEGER,
    segment_label TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (project_id) REFERENCES study_projects(id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES project_files(id) ON DELETE SET NULL
);

CREATE INDEX idx_summaries_project_id ON summaries(project_id);
CREATE INDEX idx_summaries_status ON summaries(status);
