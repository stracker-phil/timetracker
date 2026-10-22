-- A recording holds one or more spans of time (ADR 0020). Each existing recording keeps its time as one
-- span, under the recording's id.
CREATE TABLE recording_spans (
  id TEXT PRIMARY KEY,
  recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
  start INTEGER NOT NULL,
  "end" INTEGER                    -- NULL only for the running timer's span
);
CREATE INDEX recording_spans_start ON recording_spans(start);
CREATE INDEX recording_spans_recording ON recording_spans(recording_id, start);

INSERT INTO recording_spans (id, recording_id, start, "end")
  SELECT id, id, start, "end" FROM recordings;

DROP INDEX recordings_start;
ALTER TABLE recordings DROP COLUMN start;
ALTER TABLE recordings DROP COLUMN "end";
