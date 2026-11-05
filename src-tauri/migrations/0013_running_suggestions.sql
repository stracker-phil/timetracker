-- A timer started by another app runs as a stored suggestion (ADR 0041): `open_start` is where its open span
-- starts, NULL once stopped. At most one suggestion runs at a time.
ALTER TABLE stored_suggestions ADD COLUMN open_start INTEGER;
CREATE UNIQUE INDEX stored_suggestions_one_timer ON stored_suggestions((1)) WHERE open_start IS NOT NULL;
