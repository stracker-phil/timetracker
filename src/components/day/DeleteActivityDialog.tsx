import { useEffect, useState } from "react";
import { api, toAppError, type ActivityCount } from "../../api";
import { formatDayLabel } from "../../lib/day";
import { formatClock, formatDay, plural } from "../../lib/format";
import { fail } from "../../state/requests";
import { useStore } from "../../state/store";
import { FormField } from "../Field";
import { Modal } from "../Modal";

interface Props {
  day: string;
  /** The day's bounds, from its Day view. */
  bounds: [number, number];
  /** Two date-time fields instead of the whole day. */
  range: boolean;
}

/** `2026-10-21T09:30`, the local value of a `datetime-local` input. */
const toInput = (ms: number) => `${formatDay(ms)}T${formatClock(ms)}`;
const fromInput = (value: string) => (value ? new Date(value).getTime() : NaN);

const spans = (count: ActivityCount) => `${plural(count.activities, "activity span", "activity spans")} and ${plural(count.away, "away span", "away spans")}`;

function rangeLabel(from: number, to: number) {
  const at = (ms: number) => `${formatDayLabel(formatDay(ms))} ${formatClock(ms)}`;
  return `${at(from)} – ${at(to)}`;
}

function undoDelete() {
  api
    .activityDeleteUndo()
    .then((restored) => useStore.getState().toast("success", `Restored ${spans(restored)}`))
    .catch((e: unknown) => fail(e, "Couldn't undo the delete"));
}

/** Deletes the activity and away spans of a day or range; the toast offers Undo while Rust keeps the rows. */
export function DeleteActivityDialog({ day, bounds, range }: Props) {
  const close = useStore((s) => s.closeDialog);
  const [fromText, setFromText] = useState(() => toInput(bounds[0]));
  const [toText, setToText] = useState(() => toInput(bounds[1]));
  const [from, to] = range ? [fromInput(fromText), fromInput(toText)] : bounds;
  const [count, setCount] = useState<ActivityCount | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const valid = Number.isFinite(from) && Number.isFinite(to);

  useEffect(() => {
    if (!valid) return;
    let current = true;
    const show = (c: ActivityCount | null, message: string | null) => {
      if (!current) return;
      setCount(c);
      setError(message);
    };
    api.activityCount(from, to).then(
      (c) => show(c, null),
      (e: unknown) => show(null, toAppError(e).message),
    );
    return () => {
      current = false;
    };
  }, [from, to, valid]);

  const remove = async () => {
    setBusy(true);
    try {
      const deleted = await api.activityDelete(from, to);
      close();
      const what = range ? rangeLabel(from, to) : formatDayLabel(day);
      useStore.getState().toast("success", `Deleted activity for ${what}`, { label: "Undo", run: undoDelete, ttlMs: Math.max(0, deleted.undoUntil - Date.now()) });
    } catch (e) {
      setBusy(false);
      setError(toAppError(e).message);
    }
  };

  return (
    <Modal onClose={close} labelledBy="delete-activity-title">
      <h2 id="delete-activity-title">{range ? "Delete activity in a range" : `Delete activity for ${formatDayLabel(day)}`}</h2>
      {range && (
        <div className="form-row" style={{ gap: "0.615rem" }}>
          <FormField label="From" htmlFor="delete-activity-from">
            <input id="delete-activity-from" className="input mono" type="datetime-local" value={fromText} onChange={(e) => setFromText(e.target.value)} />
          </FormField>
          <FormField label="To" htmlFor="delete-activity-to">
            <input id="delete-activity-to" className="input mono" type="datetime-local" value={toText} onChange={(e) => setToText(e.target.value)} />
          </FormField>
        </div>
      )}
      <p className="muted" style={{ margin: 0 }}>
        {count && !error ? `Deletes ${spans(count)}. ` : ""}Spans that cross the range are deleted entirely. Recordings are not affected.
      </p>
      {error && (
        <p className="form-field-error" role="alert" style={{ margin: 0 }}>
          {error}
        </p>
      )}
      <div className="actions">
        <button type="button" className="btn" onClick={close}>
          Cancel
        </button>
        <button type="button" className="btn btn-danger" disabled={busy || !valid || !!error || !count} onClick={() => void remove()}>
          Delete
        </button>
      </div>
    </Modal>
  );
}
