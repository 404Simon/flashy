CREATE INDEX idx_study_projects_owner_created
    ON study_projects(user_id, created_at DESC, id DESC);
CREATE INDEX idx_project_files_project_created
    ON project_files(project_id, created_at DESC, id DESC);
CREATE INDEX idx_flashcard_decks_project_created
    ON flashcard_decks(project_id, created_at DESC, id DESC);
CREATE INDEX idx_flashcards_deck_created
    ON flashcards(deck_id, created_at ASC, id ASC);
CREATE INDEX idx_summaries_project_created
    ON summaries(project_id, created_at DESC, id DESC);
