-- Time tracking only: no billing fields.
ALTER TABLE projects DROP COLUMN billable;
ALTER TABLE projects DROP COLUMN hourly_rate;
ALTER TABLE recordings DROP COLUMN billable;
