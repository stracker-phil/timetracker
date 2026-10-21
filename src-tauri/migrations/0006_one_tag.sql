-- A recording and a rule have at most one tag (ADR 0019). Of several, the first by name (the order Settings
-- shows) stays; `db::one_tag` backs up the data before this runs when any row loses a tag.
ALTER TABLE recordings ADD COLUMN tag_id TEXT REFERENCES tags(id) ON DELETE SET NULL;

UPDATE recordings SET tag_id = (
  SELECT t.id FROM recording_tags rt JOIN tags t ON t.id = rt.tag_id
  WHERE rt.recording_id = recordings.id ORDER BY t.name COLLATE NOCASE, t.name LIMIT 1);

DROP TABLE recording_tags;

ALTER TABLE rules ADD COLUMN tag_id TEXT REFERENCES tags(id) ON DELETE SET NULL;

UPDATE rules SET tag_id = (
  SELECT t.id FROM json_each(rules.tag_ids) j JOIN tags t ON t.id = j.value
  ORDER BY t.name COLLATE NOCASE, t.name LIMIT 1);

ALTER TABLE rules DROP COLUMN tag_ids;
