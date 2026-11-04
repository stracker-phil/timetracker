/** Text and number fields for settings rows; a value Rust refuses shows its message under the field. */
import { useEffect, useId, useRef, useState, type InputHTMLAttributes, type ReactNode } from "react";
import { Row } from "./controls";

/** Saves a value; may resolve Rust's message when it refused it (`SettingsPageProps.commit`). */
type Commit = (value: string) => void | Promise<string | null>;

/** A number takes its range, which its error message names. */
type InputKind = { type?: "text"; min?: never; max?: never } | { type: "number"; min: number; max: number };

type BufferedInputProps = Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "type" | "min" | "max" | "onError"> &
  InputKind & {
    value: string;
    onCommit: Commit;
    /** The message of a refused value, or null once a value was saved or the field shows the saved one. */
    onError?: (message: string | null) => void;
  };

/** Commits on blur or ↵ (number spinner steps debounced) and keeps a refused value for the user to correct. */
export function BufferedInput({ value, onCommit, onError, ...props }: BufferedInputProps) {
  const [text, setText] = useState(value);
  const stepTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const refused = useRef<string | null>(null);
  useEffect(() => setText(value), [value]);
  useEffect(() => () => clearTimeout(stepTimer.current), []);

  const rangeError = (draft: string) => {
    if (props.type !== "number") return null;
    const { min, max } = props;
    const n = Number(draft);
    const valid = draft.trim() !== "" && Number.isInteger(n) && n >= min && n <= max;
    return valid ? null : `Enter a whole number from ${min} to ${max}.`;
  };
  const commit = async (draft: string) => {
    clearTimeout(stepTimer.current);
    if (draft === value) {
      refused.current = null;
      onError?.(null);
      return;
    }
    if (draft === refused.current) return;
    const message = rangeError(draft) ?? (await onCommit(draft)) ?? null;
    refused.current = message ? draft : null;
    onError?.(message);
  };

  return (
    <input
      {...props}
      value={text}
      onChange={(e) => {
        const next = e.target.value;
        setText(next);
        // Typing reports an inputType; spinner clicks and arrow-key steps do not.
        const typed = !!(e.nativeEvent as InputEvent).inputType;
        if (props.type !== "number" || typed || rangeError(next)) return;
        clearTimeout(stepTimer.current);
        stepTimer.current = setTimeout(() => void commit(next), 300);
      }}
      onBlur={() => void commit(text)}
      onKeyDown={(e) => {
        if (e.key === "Enter") void commit(text);
      }}
    />
  );
}

/** Rust's message for a field, in red under it. */
export function FieldError({ id, message }: { id: string; message?: string | null }) {
  if (!message) return null;
  return (
    <span id={id} className="settings-field-error selectable" role="alert">
      {message}
    </span>
  );
}

type TextFieldProps = InputKind & {
  label: string;
  description?: ReactNode;
  value: string;
  onCommit: Commit;
  suffix?: string;
  placeholder?: string;
};

/** A row with a buffered input and the error of its last refused value below it. */
export function TextField({ label, description, suffix, ...input }: TextFieldProps) {
  const id = useId();
  const errorId = `${id}-error`;
  const [error, setError] = useState<string | null>(null);
  return (
    <Row label={label} description={description} htmlFor={id}>
      <div className="settings-field">
        <div className="settings-input">
          <BufferedInput
            {...input}
            id={id}
            className={`input${input.type === "number" ? " number" : ""}`}
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? errorId : undefined}
            onError={setError}
          />
          {suffix && <span className="muted">{suffix}</span>}
        </div>
        <FieldError id={errorId} message={error} />
      </div>
    </Row>
  );
}

type NumberFieldProps = Omit<TextFieldProps, "value" | "onCommit" | "type" | "min" | "max"> & {
  value: number;
  onCommit: (value: number) => void | Promise<string | null>;
  min: number;
  max: number;
};

/** Commits only a whole number within `min` and `max`. */
export function NumberField({ value, onCommit, ...rest }: NumberFieldProps) {
  return <TextField {...rest} type="number" value={String(value)} onCommit={(text) => onCommit(Number(text))} />;
}
