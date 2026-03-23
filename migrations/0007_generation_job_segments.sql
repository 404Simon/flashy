-- Add optional segment info for generation jobs
ALTER TABLE generation_jobs ADD COLUMN segment_label TEXT;
ALTER TABLE generation_jobs ADD COLUMN segment_ranges TEXT;
