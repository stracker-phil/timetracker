import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { at } from "../../test/dayFixtures";
import { atClock, TimeField } from "./TimeField";

const STEP = 5 * 60_000;

function renderField(value: number, onChange = vi.fn()) {
  const view = render(<TimeField label="Start" value={value} step={STEP} onChange={onChange} />);
  const rerender = (next: number) => view.rerender(<TimeField label="Start" value={next} step={STEP} onChange={onChange} />);
  return { field: () => screen.getByLabelText("Start"), rerender, onChange };
}

/** A field whose parent keeps the value, as the editor does. */
function Controlled({ initial }: { initial: number }) {
  const [value, setValue] = useState(initial);
  return <TimeField label="Start" value={value} step={STEP} onChange={setValue} />;
}

describe("TimeField", () => {
  it("shows the value as HH:MM", () => {
    expect(renderField(at("09:05")).field()).toHaveValue("09:05");
  });

  it("shows a value changed from outside", () => {
    const { field, rerender } = renderField(at("09:00"));
    rerender(at("10:30"));
    expect(field()).toHaveValue("10:30");
  });

  it("shows a value changed from outside even while it is focused, unless the typed text belongs to the current value", async () => {
    const { field, rerender } = renderField(at("09:00"));
    await userEvent.clear(field());
    await userEvent.type(field(), "1");
    rerender(at("09:00"));
    expect(field()).toHaveValue("1");
    rerender(at("10:30"));
    expect(field()).toHaveValue("10:30");
    expect(field()).toHaveFocus();
  });

  it("keeps a typed time while the parent takes it over", async () => {
    render(<Controlled initial={at("09:00")} />);
    const field = screen.getByLabelText("Start");
    await userEvent.clear(field);
    await userEvent.type(field, "0845");
    expect(field).toHaveValue("0845");
  });

  it("reports each valid typed time at once", async () => {
    const { field, onChange } = renderField(at("09:00"));
    await userEvent.clear(field());
    await userEvent.type(field(), "11:15");
    expect(onChange).toHaveBeenLastCalledWith(at("11:15"));
  });

  it("reports nothing for text that is not a time and shows the value again on blur", async () => {
    const { field, onChange } = renderField(at("09:00"));
    await userEvent.clear(field());
    await userEvent.type(field(), "later");
    expect(field()).toHaveValue("later");
    await userEvent.tab();
    expect(onChange).not.toHaveBeenCalled();
    expect(field()).toHaveValue("09:00");
  });

  it("shows the formatted value on blur, however the time was typed", async () => {
    render(<Controlled initial={at("09:00")} />);
    const field = screen.getByLabelText("Start");
    await userEvent.clear(field);
    await userEvent.type(field, "0845");
    await userEvent.tab();
    expect(field).toHaveValue("08:45");
  });

  it.each([
    ["{ArrowUp}", "09:05"],
    ["{ArrowDown}", "08:55"],
  ])("moves by the step on %s", async (key, shown) => {
    const onChange = vi.fn();
    const { field } = renderField(at("09:00"), onChange);
    await userEvent.type(field(), key);
    expect(onChange).toHaveBeenCalledWith(at(shown));
  });

  it("shows the stepped time when the parent takes it over", async () => {
    render(<Controlled initial={at("09:00")} />);
    const field = screen.getByLabelText("Start");
    await userEvent.type(field, "{ArrowUp}{ArrowUp}");
    expect(field).toHaveValue("09:10");
  });
});

describe("atClock", () => {
  it.each([
    ["09:30", "09:30"],
    ["0930", "09:30"],
    ["9:30", "09:30"],
    [" 17:05 ", "17:05"],
  ])("reads %j as %s on the day of the given time", (text, time) => {
    expect(atClock(at("00:00", "2026-03-11"), text)).toBe(at(time, "2026-03-11"));
  });

  it.each(["", "later", "24:00", "12:60", "1:5"])("reads %j as no time", (text) => {
    expect(atClock(at("00:00"), text)).toBeNull();
  });
});
