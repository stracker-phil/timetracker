import { useState } from "react";
import { useStore } from "../state/store";
import { Modal } from "./Modal";

interface Props {
  title: string;
  label: string;
  initial: string;
  confirmLabel: string;
  onSubmit: (value: string) => void;
}

/** Asks for one line of text; Save stays disabled while it is blank. */
export function PromptDialog({ title, label, initial, confirmLabel, onSubmit }: Props) {
  const close = useStore((s) => s.closeDialog);
  const [value, setValue] = useState(initial);
  const blank = value.trim() === "";
  const submit = () => {
    if (blank) return;
    close();
    onSubmit(value.trim());
  };
  return (
    <Modal onClose={close} labelledBy="prompt-title">
      <h2 id="prompt-title">{title}</h2>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <input className="input" aria-label={label} placeholder={label} autoFocus value={value} onChange={(e) => setValue(e.target.value)} />
        <div className="actions">
          <button type="button" className="btn" onClick={close}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={blank}>
            {confirmLabel}
          </button>
        </div>
      </form>
    </Modal>
  );
}
