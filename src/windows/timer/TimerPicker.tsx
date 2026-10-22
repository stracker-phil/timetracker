import { useEffect, useState } from "react";
import { Play, Square } from "lucide-react";
import { api, type RecentTimer, type RunningTimer as Running } from "../../api";
import { AccentDot } from "../../components/Accent";
import { NoteInput } from "../../components/NoteInput";
import { ProjectPicker } from "../../components/pickers/ProjectPicker";
import { TagPicker } from "../../components/pickers/TagPicker";
import { formatHm } from "../../lib/format";
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

function RunningTimer({ running, onAnother, onStop }: { running: Running; onAnother: () => void; onStop: () => void }) {
  return (
    <>
      <h1>Timer</h1>
      <div className="timer-running">
        <AccentDot accent={running.color} shade={running.shade} />
        <span className="grow-text">{running.projectName}</span>
        <span className="mono">{formatHm(running.elapsedMs)}</span>
      </div>
      {running.recording.note && <p className="o2">{running.recording.note}</p>}
      <div className="actions">
        <button type="button" className="btn" onClick={onAnother}>
          Start another…
        </button>
        <button type="button" className="btn btn-primary" autoFocus onClick={onStop}>
          <Square size="0.923rem" /> Stop
        </button>
      </div>
    </>
  );
}

function StartForm({ onStart }: { onStart: Start }) {
  const [projectId, setProjectId] = useState<string | null>(null);
  const [note, setNote] = useState("");
  const [tagId, setTagId] = useState<string | null>(null);
  return (
    <form
      className="timer-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (projectId) onStart(projectId, note.trim(), tagId);
      }}
    >
      <h1>Start timer</h1>
      <ProjectPicker value={projectId} onChange={setProjectId} autoFocus />
      <NoteInput aria-label="Note" placeholder="Note" maxLength={500} value={note} onChange={setNote} />
      <TagPicker value={tagId} onChange={setTagId} />
      <div className="actions">
        <button type="button" className="btn" onClick={close}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" disabled={!projectId}>
          <Play size="0.923rem" /> Start
        </button>
      </div>
    </form>
  );
}

function RecentTimers({ recent, onStart }: { recent: RecentTimer[]; onStart: Start }) {
  return (
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
  );
}

/** The small window under the tray icon: start a timer (project, note, tag), or stop the running one. */
export function TimerPicker() {
  const recent = useRecentTimers();
  const running = useStore((s) => s.timer?.running);
  // A new form after every start, so it opens empty next time.
  const [formKey, setFormKey] = useState(0);
  const [another, setAnother] = useState(false);

  const start: Start = (projectId, note, tagId) =>
    void useStore
      .getState()
      .startTimer(projectId, note, tagId)
      .then((started) => {
        if (!started) return;
        setFormKey((k) => k + 1);
        setAnother(false);
        close();
      });
  const stop = () => void useStore.getState().stopTimer().then(close);

  return (
    <div
      className="timer-picker"
      onKeyDown={(e) => {
        if (e.key === "Escape" && !e.defaultPrevented) close();
      }}
    >
      {running && !another ? <RunningTimer running={running} onAnother={() => setAnother(true)} onStop={stop} /> : <StartForm key={formKey} onStart={start} />}
      {!running && recent.length > 0 && <RecentTimers recent={recent} onStart={start} />}
    </div>
  );
}
