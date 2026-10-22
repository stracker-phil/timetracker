import type { InputHTMLAttributes } from "react";

export const NOTE_PLACEHOLDER = "What did you work on?";

const oneLine = (text: string) => text.replace(/[\r\n]+/g, " ");

type Props = Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "onChange" | "onPaste"> & {
  value: string;
  onChange: (note: string) => void;
};

/** A recording's one-line note: line breaks, typed or pasted, become spaces. */
export function NoteInput({ value, onChange, ...rest }: Props) {
  return (
    <input
      {...rest}
      className="input"
      value={value}
      onChange={(e) => onChange(oneLine(e.target.value))}
      onPaste={(e) => {
        // A text field drops pasted line breaks, joining the words; spaces keep them apart.
        e.preventDefault();
        const field = e.currentTarget;
        const pasted = oneLine(e.clipboardData.getData("text"));
        const [from, to] = [field.selectionStart ?? field.value.length, field.selectionEnd ?? field.value.length];
        onChange(field.value.slice(0, from) + pasted + field.value.slice(to));
      }}
    />
  );
}
