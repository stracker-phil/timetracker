-- Billable is a plain yes/no per project; a recording is billable through its project. Existing projects
-- start as not billable.
ALTER TABLE projects ADD COLUMN billable INTEGER NOT NULL DEFAULT 0;
