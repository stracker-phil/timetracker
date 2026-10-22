import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { api, toAppError, type AppError, type AwayPrompt as Prompt, type AwayReason, type ProjectListItem, type Recording } from "../../api";
import { FormField } from "../../components/Field";
import { NOTE_PLACEHOLDER, NoteInput } from "../../components/NoteInput";
import { TimeField, typedTimesSettled } from "../../components/day/TimeField";
import { ProjectPicker } from "../../components/pickers/ProjectPicker";
import { TagPicker } from "../../components/pickers/TagPicker";
import { formatClock, formatDuration, MINUTE_MS } from "../../lib/format";
import { spansText } from "../../lib/recordings";
import { applyAppearance, systemIsDark } from "../../lib/theme";
import { useApiResult } from "../../lib/useApiResult";
import { useStore } from "../../state/store";

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
        if (settings) applyAppearance(settings, systemIsDark());
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
  const spans = recordings.map((r) => `${spansText(r.spans)} (${nameOf(r.projectId)})`).join(", ");
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
  // The latest times, which Record reads after the fields' typing has settled.
  const times = useRef({ start, end: endValue });
  const changeStart = (ms: number) => {
    times.current.start = ms;
    setStart(ms);
  };
  const changeEnd = (ms: number) => {
    times.current.end = ms;
    setEndValue(ms);
  };
  const [note, setNote] = useState("");
  const [tagId, setTagId] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [failure, setFailure] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const ids = { start: useId(), end: useId(), note: useId() };

  const projectId = picked === undefined ? lastProjectId : picked;
  // Rust rolls an end at or before the start to the next day.
  const draftSpans = () => api.recordingDraftSpans([{ start, end: endValue }]);
  const { data: draft } = useApiResult(`away-span:${start}-${endValue}`, draftSpans, { keepData: true });

  async function record() {
    if (!projectId) {
      setErrors({ projectId: "Pick a project" });
      return;
    }
    setErrors({});
    setFailure(null);
    setSaving(true);
    try {
      await typedTimesSettled();
      const { spans } = await api.recordingDraftSpans([times.current]);
      await api.awayPromptAnswer({ projectId, spans, note: note.trim(), tagId });
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
      <FormField label="Note" htmlFor={ids.note} error={errors.note}>
        <NoteInput id={ids.note} placeholder={NOTE_PLACEHOLDER} autoFocus maxLength={500} value={note} onChange={setNote} />
      </FormField>
      <FormField label="Project" error={errors.projectId}>
        <ProjectPicker value={projectId} onChange={setPicked} />
      </FormField>
      <FormField label="Tag" error={errors.tagId}>
        <TagPicker value={tagId} onChange={setTagId} />
      </FormField>
      <div className="away-times">
        <FormField label="From" htmlFor={ids.start} error={errors.start}>
          <TimeField id={ids.start} value={start} step={step} onChange={changeStart} />
        </FormField>
        <FormField label="To" htmlFor={ids.end} error={errors.end}>
          <TimeField id={ids.end} value={endValue} step={step} onChange={changeEnd} />
        </FormField>
        {draft && <span className="away-length o2">{formatDuration(draft.totalMs)}</span>}
      </div>
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
  const { span } = prompt;
  return (
    <>
      <header>
        <h1>You were away {formatDuration(prompt.awayMs)}</h1>
        <p className="o2 tabular">
          {formatClock(span.start)}–{formatClock(prompt.endedAt)} · {REASONS[span.reason]}
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
