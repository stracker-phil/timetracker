-- A tag may carry one emoji, shown before its name; existing tags have none.
ALTER TABLE tags ADD COLUMN emoji TEXT;
