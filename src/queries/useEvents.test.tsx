import { act, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, type Settings } from "../api";
import { bindingFor, installCommands } from "../commands";
import { resetStore } from "../state/store";
import { emit } from "../test/apiMock";
import { trackerStatus } from "../test/data";
import { dayView } from "../test/dayFixtures";
import { client } from "../test/projectFixtures";
import { queryData, renderWithClient, SETTINGS_KEY } from "../test/queries";
import { rule } from "../test/ruleFixtures";
import { settings } from "../test/settings";
import { runningStatus } from "../test/timerFixtures";
import { weekView } from "../test/calendarFixtures";
import { usePeriodTotals, useWeekView } from "./calendar";
import { useDayView } from "./day";
import { useProjectLists } from "./projects";
import { useRules } from "./rules";
import { useKeybindings, useSettings } from "./settings";
import { useTimer, useTracker } from "./status";
import { useEvents } from "./useEvents";

const mocked = vi.mocked(api, true);

const DAY_A = "2026-03-10";
const DAY_B = "2026-03-11";

/** What the mocked Rust answers; the reads show it, so a refetch is visible as the new value. */
let answer = "old";

function Reads() {
  useEvents();
  const dayA = useDayView(DAY_A).data;
  const dayB = useDayView(DAY_B).data;
  const week = useWeekView(DAY_A).data;
  const totals = usePeriodTotals({ kind: "today" }).data;
  const { clients } = useProjectLists();
  const rules = useRules();
  const config = useSettings();
  const bindings = useKeybindings();
  const timer = useTimer();
  const tracker = useTracker();
  return (
    <ul>
      <li>dayA: {dayA?.label}</li>
      <li>dayB: {dayB?.label}</li>
      <li>week: {week?.label}</li>
      <li>totals: {totals?.label}</li>
      <li>clients: {clients.map((c) => c.name).join(",")}</li>
      <li>rules: {rules.map((r) => r.name).join(",")}</li>
      <li>settings: {config?.textSize}</li>
      <li>bindings: {Object.keys(bindings).join(",")}</li>
      <li>timer: {timer ? (timer.running?.projectName ?? "none") : ""}</li>
      <li>tracker: {tracker?.text}</li>
    </ul>
  );
}

const READS = ["dayA", "dayB", "week", "totals", "clients", "rules"] as const;

const flush = () => act(() => new Promise((resolve) => setTimeout(resolve, 0)));

async function showReads() {
  renderWithClient(<Reads />);
  for (const name of READS) await screen.findByText(`${name}: old`);
  await screen.findByText("settings: medium");
}

describe("useEvents", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    answer = "old";
    resetStore();
    installCommands();
    mocked.dayView.mockImplementation(async (day) => dayView({ day, label: answer }));
    mocked.weekView.mockImplementation(async () => weekView({ label: answer }));
    mocked.periodTotals.mockImplementation(async () => ({ label: answer, recordedMs: 0, billableMs: 0, billablePercent: null }));
    mocked.clientsList.mockImplementation(async () => [client({ name: answer })]);
    mocked.rulesList.mockImplementation(async () => [rule({ name: answer })]);
    mocked.getSettings.mockImplementation(async () => settings({ textSize: answer === "old" ? "medium" : "large" }));
    mocked.timerStatus.mockResolvedValue({ running: null });
    mocked.trackerStatus.mockResolvedValue(trackerStatus());
  });

  describe("data_changed", () => {
    it.each<[string, "recordings" | "activity" | "projects" | "tags" | "rules" | "settings", string | null, (typeof READS)[number][]]>([
      ["recordings of one day", "recordings", DAY_A, ["dayA", "week", "totals", "clients"]],
      ["recordings of the other day", "recordings", DAY_B, ["dayB", "week", "totals", "clients"]],
      ["recordings of unknown days", "recordings", null, ["dayA", "dayB", "week", "totals", "clients"]],
      ["activity of one day", "activity", DAY_A, ["dayA", "week", "totals"]],
      ["activity of unknown days", "activity", null, ["dayA", "dayB", "week", "totals"]],
      ["projects", "projects", null, ["dayA", "dayB", "week", "totals", "clients", "rules"]],
      ["tags", "tags", null, ["dayA", "dayB", "week", "clients", "rules"]],
      ["rules", "rules", null, ["dayA", "dayB", "rules"]],
      ["settings", "settings", null, ["dayA", "dayB", "week", "totals"]],
    ])("%s refetches exactly the reads it makes stale", async (_, kind, day, refreshed) => {
      await showReads();
      answer = "new";
      emit("data_changed", { kind, day });
      for (const name of refreshed) await screen.findByText(`${name}: new`);
      await flush();
      for (const name of READS) expect(screen.getByText(`${name}: ${refreshed.includes(name) ? "new" : "old"}`)).toBeInTheDocument();
    });

    it("refetches the settings after a settings change", async () => {
      await showReads();
      answer = "new";
      emit("data_changed", { kind: "settings", day: null });
      expect(await screen.findByText("settings: large")).toBeInTheDocument();
    });

    it("asks Rust for the changed day by its date", async () => {
      await showReads();
      mocked.dayView.mockClear();
      emit("data_changed", { kind: "recordings", day: DAY_B });
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenCalledWith(DAY_B));
      expect(mocked.dayView).not.toHaveBeenCalledWith(DAY_A);
    });
  });

  describe("events that carry their value", () => {
    it("settings_changed shows the new settings without asking Rust again", async () => {
      await showReads();
      emit("settings_changed", settings({ textSize: "small" }));
      expect(await screen.findByText("settings: small")).toBeInTheDocument();
      await flush();
      expect(screen.getByText("settings: small")).toBeInTheDocument();
    });

    it("keybindings_changed shows the new shortcuts and applies them to the commands", async () => {
      await showReads();
      expect(bindingFor("view.week")).not.toBeNull();
      emit("keybindings_changed", { "view.week": null });
      expect(await screen.findByText("bindings: view.week")).toBeInTheDocument();
      await flush();
      expect(screen.getByText("bindings: view.week")).toBeInTheDocument();
      expect(bindingFor("view.week")).toBeNull();
    });

    it("timer_status shows the running timer without asking Rust again", async () => {
      await showReads();
      await screen.findByText("timer: none");
      emit("timer_status", runningStatus({ projectName: "Research" }));
      expect(await screen.findByText("timer: Research")).toBeInTheDocument();
      await flush();
      expect(screen.getByText("timer: Research")).toBeInTheDocument();
    });

    it("tracker_status shows the new status without asking Rust again", async () => {
      await showReads();
      await screen.findByText("tracker: Tracking");
      emit("tracker_status", trackerStatus({ state: { kind: "paused", until: null }, text: "Paused" }));
      expect(await screen.findByText("tracker: Paused")).toBeInTheDocument();
      await flush();
      expect(screen.getByText("tracker: Paused")).toBeInTheDocument();
    });
  });

  it("stops following Rust once unmounted", async () => {
    const { unmount } = renderWithClient(<Reads />);
    await screen.findByText("settings: medium");
    unmount();
    await flush();
    emit("settings_changed", settings({ textSize: "small" }));
    expect(queryData<Settings>(SETTINGS_KEY)?.textSize).toBe("medium");
  });
});
