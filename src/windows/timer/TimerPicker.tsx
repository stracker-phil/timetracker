import { useEffect, useId, useState } from "react";
import { Play, Square } from "lucide-react";
import { api, type RecentTimer, type RunningTimer as Running } from "../../api";
import { AccentDot } from "../../components/Accent";
import { FormField } from "../../components/Field";
import { NOTE_PLACEHOLDER, NoteInput } from "../../components/NoteInput";
import { ProjectPicker } from "../../components/pickers/ProjectPicker";
import { TagPicker } from "../../components/pickers/TagPicker";
import { formatDuration } from "../../lib/format";
import { applyAppearance, systemIsDark } from "../../lib/theme";
import { useStore } from "../../state/store";

type Start = (projectId: string, note: string, tagId: string | null) => void;

const close = () => void api.timerPickerClose();

/** The window's data, reloaded whenever it gains focus (it is hidden, not closed, between uses) and when the
 *  timer changes; returns the recent timers. */
function useRecentTimers(): RecentTimer[] {
  const [recent, setRecent] = useState<RecentTimer[]>([]);
  useEffect(() => {
    const s = useStore.getState();
    const loadRecent = () => void api.timersRecent().then(setRecent, () => setRecent([]));
    const refresh = () => {
      void s.loadSettings().then(() => {
        const settings = useStore.getState().settings;
        if (settings) applyAppearance(settings, systemIsDark());
      });
      void s.loadProjects();
      void s.loadTimer();
      loadRecent();
    };
    refresh();
    const unlisten = api.onTimerStatus((status) => {
      s.setTimerStatus(status);
      loadRecent();
    });
    window.addEventListener("focus", refresh);
    return () => {
      window.removeEventListener("focus", refresh);
      void unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);
  return recent;
}

interface Fields {
  projectId: string | null;
  note: string;
  tagId: string | null;
}

const EMPTY: Fields = { projectId: null, note: "", tagId: null };

/** Starts a timer, or edits the running one (the same fields, filled in). */
function TimerForm({ running, onSubmit, onStop }: { running?: Running; onSubmit: Start; onStop?: () => void }) {
  const initial: Fields = running ? { projectId: running.recording.projectId, note: running.recording.note, tagId: running.recording.tagId } : EMPTY;
  const [fields, setFields] = useState(initial);
  const change = (patch: Partial<Fields>) => setFields((f) => ({ ...f, ...patch }));
  const noteId = useId();
  const { projectId, note, tagId } = fields;
  return (
    <form
      className="timer-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (projectId) onSubmit(projectId, note.trim(), tagId);
      }}
    >
      {running ? (
        <h1 className="timer-running">
          <AccentDot accent={running.color} shade={running.shade} />
          <span className="grow-text">Timer running</span>
          <span className="timer-elapsed">{formatDuration(running.elapsedMs)}</span>
        </h1>
      ) : (
        <h1>Start Timer</h1>
      )}
      <FormField label="Note" htmlFor={noteId}>
        <NoteInput
          id={noteId}
          placeholder={NOTE_PLACEHOLDER}
          autoFocus
          maxLength={500}
          value={note}
          onChange={(next) => change({ note: next })}
          // Selected, so a typed note replaces the working one.
          onFocus={running ? (e) => e.currentTarget.select() : undefined}
        />
      </FormField>
      <FormField label="Project">
        <ProjectPicker value={projectId} onChange={(next) => change({ projectId: next })} />
      </FormField>
      <FormField label="Tag">
        <TagPicker value={tagId} onChange={(next) => change({ tagId: next })} />
      </FormField>
      <div className="actions">
        {onStop && (
          <button type="button" className="btn timer-stop" onClick={onStop}>
            <Square size="0.923rem" /> Stop
          </button>
        )}
        <button type="button" className="btn" onClick={close}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" disabled={!projectId}>
          {running ? (
            "Save"
          ) : (
            <>
              <Play size="0.923rem" /> Start
            </>
          )}
        </button>
      </div>
    </form>
  );
}

/** Counts the window's openings (focus): it is hidden, not closed, between uses. */
function useOpenings(): number {
  const [opened, setOpened] = useState(0);
  useEffect(() => {
    const count = () => setOpened((n) => n + 1);
    window.addEventListener("focus", count);
    return () => window.removeEventListener("focus", count);
  }, []);
  return opened;
}

function RecentTimers({ recent, onStart }: { recent: RecentTimer[]; onStart: Start }) {
  return (
    <>
      <span className="timer-recent-label" aria-hidden="true">
        Recent
      </span>
      <ul className="timer-recent" aria-label="Recent timers">
        {recent.map((t) => (
          <li key={`${t.projectId}:${t.note}:${t.tagId ?? ""}`}>
            <button type="button" onClick={() => onStart(t.projectId, t.note, t.tagId)}>
              <AccentDot accent={t.color} shade={t.shade} />
              <span className="grow-text">{t.note ? `${t.projectName} · ${t.note}` : t.projectName}</span>
            </button>
          </li>
        ))}
      </ul>
    </>
  );
}

/** The small window under the tray icon: start a timer, or edit or stop the running one. */
export function TimerPicker() {
  const recent = useRecentTimers();
  const running = useStore((s) => s.timer?.running);
  const opened = useOpenings();
  // A new form after every start, so it opens empty next time.
  const [formKey, setFormKey] = useState(0);

  const start: Start = (projectId, note, tagId) =>
    void useStore
      .getState()
      .startTimer(projectId, note, tagId)
      .then((started) => {
        if (!started) return;
        setFormKey((k) => k + 1);
        close();
      });
  const save: Start = (projectId, note, tagId) =>
    void useStore
      .getState()
      .updateTimer(projectId, note, tagId)
      .then((saved) => saved && close());
  const stop = () => void useStore.getState().stopTimer().then(close);

  return (
    <div
      className="timer-picker"
      onKeyDown={(e) => {
        if (e.key === "Escape" && !e.defaultPrevented) close();
      }}
    >
      {running ? (
        // Filled anew on every opening and after every change, never while typing (the clock ticks).
        <TimerForm key={`${running.recording.id}:${running.recording.updatedAt}:${opened}`} running={running} onSubmit={save} onStop={stop} />
      ) : (
        <TimerForm key={formKey} onSubmit={start} />
      )}
      {!running && recent.length > 0 && <RecentTimers recent={recent} onStart={start} />}
    </div>
  );
}
