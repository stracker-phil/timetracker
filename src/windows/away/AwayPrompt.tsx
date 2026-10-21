import { useEffect, useId, useState, type KeyboardEvent } from "react";
import { api, toAppError, type AppError, type AwayPrompt as Prompt, type AwayReason, type ProjectListItem, type Recording } from "../../api";
import { FormField } from "../../components/Field";
import { NoteInput } from "../../components/NoteInput";
import { TimeField } from "../../components/day/TimeField";
import { ProjectPicker } from "../../components/pickers/ProjectPicker";
import { TagPicker } from "../../components/pickers/TagPicker";
import { formatClock, formatDuration, MINUTE_MS } from "../../lib/format";
import { applyAppearance } from "../../lib/theme";
import { useStore } from "../../state/store";

const DAY_MS = 24 * 60 * MINUTE_MS;

const REASONS: Record<AwayReason, string> = { idle: "idle", sleep: "asleep", lock: "locked", paused: "paused" };

const skip = () => void api.awayPromptSkip();

/** The open prompt (undefined while loading), reloaded whenever the window gains focus and when a newer away
 *  span replaces it. */
function usePrompt(): [Prompt | null | undefined, () => void] {
  const [prompt, setPrompt] = useState<Prompt | null | undefined>(undefined);
  const reload = () => void api.awayPromptCurrent().then(setPrompt, () => setPrompt(null));
  useEffect(() => {
    const s = useStore.getState();
    const loadAppearance = () =>
      void s.loadSettings().then(() => {
        const settings = useStore.getState().settings;
        if (settings) applyAppearance(settings);
      });
    const refresh = () => {
      loadAppearance();
      void s.loadProjects();
      reload();
    };
    refresh();
    const unlisten = api.onAwayPromptChanged(setPrompt);
    window.addEventListener("focus", refresh);
    return () => {
      window.removeEventListener("focus", refresh);
      void unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);
  return [prompt, reload];
}

function overlapMessage(recordings: Recording[], projects: ProjectListItem[]): string {
  const nameOf = (id: string) => projects.find((p) => p.project.id === id)?.project.name ?? "Unknown project";
  const spans = recordings.map((r) => `${formatClock(r.start)}–${r.end === null ? "now" : formatClock(r.end)} (${nameOf(r.projectId)})`).join(", ");
  return recordings.length === 1 ? `A recording already covers ${spans}` : `Recordings already cover ${spans}`;
}

type AnswerFailure = { kind: "gone" } | { kind: "field"; path: string; message: string } | { kind: "message"; message: string; log: boolean };

/** What a failed answer shows: an overlap or an unexpected error inline, a field error under its field; a
 *  prompt that is gone reloads. */
function answerFailure(error: AppError, projects: ProjectListItem[]): AnswerFailure {
  if (error.kind === "overlap") {
    const recordings = Array.isArray(error.details) ? (error.details as Recording[]) : [];
    return { kind: "message", message: overlapMessage(recordings, projects), log: false };
  }
  if (error.kind === "not_found") return { kind: "gone" };
  if (error.field) return { kind: "field", ...error.field };
  return { kind: "message", message: error.message, log: true };
}

function AwayForm({ prompt, onGone }: { prompt: Prompt; onGone: () => void }) {
  const step = useStore((s) => s.settings?.timeline.snapMinutes ?? 5) * MINUTE_MS;
  const lastProjectId = useStore((s) => s.recent[0]?.id ?? null);
  const projects = useStore((s) => s.projects);
  // Undefined until picked, so the last used project can arrive after the prompt.
  const [picked, setPicked] = useState<string | null | undefined>(undefined);
  const [start, setStart] = useState(prompt.suggested.start);
  const [endValue, setEndValue] = useState(prompt.suggested.end);
  const [note, setNote] = useState("");
  const [tagIds, setTagIds] = useState<string[]>([]);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [failure, setFailure] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const ids = { start: useId(), end: useId(), note: useId() };

  const projectId = picked === undefined ? lastProjectId : picked;
  // An end at or before the start is on the next day.
  const end = endValue > start ? endValue : endValue + DAY_MS;

  async function record() {
    if (!projectId) {
      setErrors({ projectId: "Pick a project" });
      return;
    }
    setErrors({});
    setFailure(null);
    setSaving(true);
    try {
      await api.awayPromptAnswer({ projectId, start, end, note: note.trim(), tagIds });
    } catch (e) {
      const failed = answerFailure(toAppError(e), projects);
      if (failed.kind === "gone") onGone();
      else if (failed.kind === "field") setErrors({ [failed.path]: failed.message });
      else {
        if (failed.log) void api.logError(`Away prompt: ${failed.message}`).catch(() => {});
        setFailure(failed.message);
      }
    } finally {
      setSaving(false);
    }
  }

  return (
    <form
      className="away-form"
      onSubmit={(e) => {
        e.preventDefault();
        void record();
      }}
    >
      <FormField label="Project" error={errors.projectId}>
        <ProjectPicker value={projectId} autoFocus onChange={setPicked} />
      </FormField>
      <div className="recording-times">
        <FormField label="From" htmlFor={ids.start} error={errors.start}>
          <TimeField id={ids.start} value={start} step={step} onChange={setStart} />
        </FormField>
        <FormField label="To" htmlFor={ids.end} error={errors.end}>
          <TimeField id={ids.end} value={endValue} step={step} onChange={setEndValue} />
        </FormField>
        <span className="recording-length mono o2">{formatDuration(end - start)}</span>
      </div>
      <FormField label="Note" htmlFor={ids.note} error={errors.note}>
        <NoteInput id={ids.note} maxLength={500} value={note} onChange={setNote} />
      </FormField>
      <FormField label="Tags" error={errors.tagIds}>
        <TagPicker value={tagIds} onChange={setTagIds} />
      </FormField>
      {failure && (
        <p className="away-failure" role="alert">
          {failure}
        </p>
      )}
      <div className="actions">
        <button type="button" className="btn" title="Skip (Esc)" onClick={skip}>
          Skip
        </button>
        <button type="submit" className="btn btn-primary" title="Record (↵)" disabled={saving}>
          Record
        </button>
      </div>
    </form>
  );
}

function NothingToRecord() {
  return (
    <>
      <p className="away-empty o2">Nothing to record</p>
      <div className="actions">
        <button type="button" className="btn btn-primary" autoFocus onClick={skip}>
          Close
        </button>
      </div>
    </>
  );
}

function PromptBody({ prompt, onGone }: { prompt: Prompt; onGone: () => void }) {
  const { span, suggested } = prompt;
  const end = span.end ?? suggested.end;
  return (
    <>
      <header>
        <h1>You were away {formatDuration(end - span.start)}</h1>
        <p className="o2 mono">
          {formatClock(span.start)}–{formatClock(end)} · {REASONS[span.reason]}
        </p>
      </header>
      <AwayForm key={span.id} prompt={prompt} onGone={onGone} />
    </>
  );
}

/** The window after a long away span: what the user did meanwhile, recorded for a project, or skipped. */
export function AwayPrompt() {
  const [prompt, reload] = usePrompt();
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape" && !e.defaultPrevented) skip();
  };
  return (
    <div className="away-prompt" onKeyDown={onKeyDown}>
      {prompt === null ? <NothingToRecord /> : prompt && <PromptBody prompt={prompt} onGone={reload} />}
    </div>
  );
}
