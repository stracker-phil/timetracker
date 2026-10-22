-- Suggestions the owner dismissed: the rule suggests nothing again in [start, end) of the local day.
CREATE TABLE dismissed_suggestions (rule_id TEXT NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
  day TEXT NOT NULL, start INTEGER NOT NULL, "end" INTEGER NOT NULL, PRIMARY KEY (rule_id, start, "end"));
CREATE INDEX dismissed_suggestions_day ON dismissed_suggestions (day);
