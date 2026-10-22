import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { at } from "../../test/dayFixtures";
import { TimeField, typedTimesSettled } from "./TimeField";

const mocked = vi.mocked(api, true);

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

/** Readings of typed text that Rust answers when told to. */
function slowReadings() {
  const pending: ((ms: number | null) => void)[] = [];
  mocked.calendarParseTime.mockImplementation(() => new Promise((resolve) => pending.push(resolve)));
  return { answer: (index: number, ms: number | null) => act(async () => pending[index](ms)) };
}

describe("TimeField", () => {
  afterEach(() => void mocked.calendarParseTime.mockReset());

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
    await act(async () => {});
    expect(field).toHaveValue("0845");
  });

  describe("typing", () => {
    it("reports the time Rust reads on the day of the value", async () => {
      mocked.calendarParseTime.mockImplementation(async (text) => (text === "x" ? at("07:07") : null));
      const { field, onChange } = renderField(at("09:00"));
      await userEvent.clear(field());
      await userEvent.type(field(), "x");
      await vi.waitFor(() => expect(onChange).toHaveBeenLastCalledWith(at("07:07")));
      expect(mocked.calendarParseTime).toHaveBeenLastCalledWith("x", at("09:00"));
    });

    it("reports each valid typed time", async () => {
      const { field, onChange } = renderField(at("09:00"));
      await userEvent.clear(field());
      await userEvent.type(field(), "11:15");
      await vi.waitFor(() => expect(onChange).toHaveBeenLastCalledWith(at("11:15")));
    });

    it("reports nothing when Rust reads no time and shows the value again on blur", async () => {
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
      await vi.waitFor(() => expect(field).toHaveValue("08:45"));
    });

    it("ignores a reading that answers after a newer one was asked", async () => {
      const { answer } = slowReadings();
      const { field, onChange } = renderField(at("09:00"));
      await userEvent.clear(field());
      await userEvent.type(field(), "1");
      await answer(1, at("10:00"));
      await answer(0, at("08:00"));
      expect(onChange).not.toHaveBeenCalledWith(at("08:00"));
      expect(onChange).toHaveBeenLastCalledWith(at("10:00"));
    });

    it("ignores a reading that answers after an arrow key stepped the time", async () => {
      const { answer } = slowReadings();
      const { field, onChange } = renderField(at("09:00"));
      await userEvent.type(field(), "x{ArrowUp}");
      await answer(0, at("12:00"));
      expect(onChange).not.toHaveBeenCalledWith(at("12:00"));
      expect(onChange).toHaveBeenCalledWith(at("09:05"));
    });
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

  describe("typedTimesSettled", () => {
    it("resolves at once when nothing is being read", async () => {
      await expect(typedTimesSettled()).resolves.toBeUndefined();
    });

    it("waits until Rust has read the typed time and the parent has taken it", async () => {
      const { answer } = slowReadings();
      const { field, onChange } = renderField(at("09:00"));
      await userEvent.type(field(), "x");
      let settled = false;
      const done = typedTimesSettled().then(() => void (settled = true));
      await new Promise((resolve) => setTimeout(resolve, 10));
      expect(settled).toBe(false);
      await answer(0, at("10:00"));
      await done;
      expect(onChange).toHaveBeenCalledWith(at("10:00"));
    });

    it("also waits for the work the parent starts for a time", async () => {
      let finish: () => void = () => {};
      const onChange = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
      const { field } = renderField(at("09:00"), onChange);
      await userEvent.type(field(), "{ArrowUp}");
      let settled = false;
      const done = typedTimesSettled().then(() => void (settled = true));
      await new Promise((resolve) => setTimeout(resolve, 10));
      expect(settled).toBe(false);
      finish();
      await done;
      expect(settled).toBe(true);
    });

    it("resolves when Rust cannot read the time", async () => {
      mocked.calendarParseTime.mockRejectedValue({ kind: "io", message: "gone", code: null });
      const { field, onChange } = renderField(at("09:00"));
      await userEvent.type(field(), "x");
      await typedTimesSettled();
      expect(onChange).not.toHaveBeenCalled();
    });
  });
});
