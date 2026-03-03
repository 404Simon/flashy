-- Add processing_status to project_files
ALTER TABLE project_files ADD COLUMN processing_status TEXT NOT NULL DEFAULT 'pending';
-- Possible values: 'pending', 'processing', 'completed', 'failed'

-- Update existing files to 'completed' since they were processed synchronously
UPDATE project_files SET processing_status = 'completed' WHERE extracted_text IS NOT NULL;
