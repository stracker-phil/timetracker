import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { installCommands } from "../../commands";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsDialog } from "./SettingsDialog";

const mocked = vi.mocked(api, true);

const APPS = [
  { bundleId: "com.apple.Safari", app: "Safari", lastSeen: 3, totalMs: 3_600_000 },
  { bundleId: "com.apple.Notes", app: "Notes", lastSeen: 2, totalMs: 600_000 },
  { bundleId: null, app: "Legacy Tool", lastSeen: 1, totalMs: 60_000 },
];

type Capture = ReturnType<typeof settings>["capture"];

function renderPage(page: "capture" | "tracker", capture: Partial<Capture> = {}) {
  const base = settings();
  resetStore({ settings: { ...base, capture: { ...base.capture, ...capture } }, dialog: { type: "settings", page } });
  installCommands();
  return render(<SettingsDialog />);
}

function lastSaved() {
  return mocked.updateSettings.mock.lastCall![0];
}

describe("Settings Capture page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.appsSeen.mockResolvedValue(APPS);
  });

  describe("capture switches", () => {
    it.each([
      ["Capture page addresses in browsers", "captureUrls"],
      ["Capture document paths", "captureDocuments"],
    ] as const)("saves %s flipped and leaves the rest of capture unchanged", async (name, key) => {
      renderPage("capture", { excludedDomains: ["bank.example.com"] });
      const toggle = screen.getByRole("switch", { name });
      expect(toggle).toBeChecked();
      await userEvent.click(toggle);
      expect(lastSaved().capture).toEqual({
        ...settings().capture,
        excludedDomains: ["bank.example.com"],
        [key]: false,
      });
    });
  });

  describe("Excluded websites", () => {
    it("lists each excluded domain", () => {
      renderPage("capture", { excludedDomains: ["bank.example.com", "mail.test"] });
      expect(screen.getByRole("button", { name: "Remove bank.example.com" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Remove mail.test" })).toBeInTheDocument();
    });

    it("saves the trimmed, lowercased domain on Enter and clears the field", async () => {
      renderPage("capture", { excludedDomains: ["mail.test"] });
      const field = screen.getByLabelText("Website to exclude");
      await userEvent.type(field, "  Bank.Example.COM {Enter}");
      expect(lastSaved().capture.excludedDomains).toEqual(["mail.test", "bank.example.com"]);
      expect(field).toHaveValue("");
    });

    it("shows an alert and saves nothing when the field is blank", async () => {
      renderPage("capture");
      await userEvent.type(screen.getByLabelText("Website to exclude"), "   {Enter}");
      expect(screen.getByRole("alert")).toHaveTextContent("Enter a domain, like bank.example.com.");
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("removes a domain from the saved list", async () => {
      renderPage("capture", { excludedDomains: ["bank.example.com", "mail.test"] });
      await userEvent.click(screen.getByRole("button", { name: "Remove bank.example.com" }));
      expect(lastSaved().capture.excludedDomains).toEqual(["mail.test"]);
    });
  });

  describe("Excluded apps", () => {
    it("says no apps are excluded when the list is empty", () => {
      renderPage("capture");
      expect(screen.getByText("No excluded apps")).toBeInTheDocument();
    });

    it("labels an excluded bundle id with the app name once it is known", async () => {
      renderPage("capture", { excludedApps: ["com.apple.safari"] });
      expect(await screen.findByRole("button", { name: "Remove Safari" })).toBeInTheDocument();
    });

    it("falls back to the bundle id for an app that was not seen recently", async () => {
      renderPage("capture", { excludedApps: ["com.example.gone"] });
      expect(await screen.findByRole("button", { name: "Remove com.example.gone" })).toBeInTheDocument();
    });

    it("saves the list without the removed app", async () => {
      renderPage("capture", { excludedApps: ["com.apple.safari", "com.apple.notes"] });
      await userEvent.click(await screen.findByRole("button", { name: "Remove Safari" }));
      expect(lastSaved().capture.excludedApps).toEqual(["com.apple.notes"]);
    });

    describe("app picker", () => {
      async function openPicker(capture: Partial<Capture> = {}) {
        renderPage("capture", capture);
        await userEvent.click(screen.getByRole("button", { name: /Add app/ }));
        const dialog = screen.getByRole("dialog", { name: "Exclude an app" });
        await within(dialog).findByText("Notes");
        return dialog;
      }

      it("lists the recently seen apps", async () => {
        const dialog = await openPicker();
        expect(within(dialog).getAllByRole("listitem")).toHaveLength(3);
        expect(within(dialog).getByText("Safari")).toBeInTheDocument();
        expect(within(dialog).getByText("Legacy Tool")).toBeInTheDocument();
      });

      it("hides apps that are already excluded", async () => {
        const dialog = await openPicker({ excludedApps: ["com.apple.safari"] });
        expect(within(dialog).queryByText("Safari")).not.toBeInTheDocument();
        expect(within(dialog).getByText("Notes")).toBeInTheDocument();
      });

      it("filters the list by the search text", async () => {
        const dialog = await openPicker();
        await userEvent.type(within(dialog).getByLabelText("Search apps"), "saf");
        expect(within(dialog).getByText("Safari")).toBeInTheDocument();
        expect(within(dialog).queryByText("Notes")).not.toBeInTheDocument();
      });

      it("says no app matches when the search finds nothing", async () => {
        const dialog = await openPicker();
        await userEvent.type(within(dialog).getByLabelText("Search apps"), "zzz");
        expect(within(dialog).getByText("No app matches.")).toBeInTheDocument();
      });

      it("adds the lowercased bundle id of the chosen app and closes the picker", async () => {
        const dialog = await openPicker({ excludedApps: ["com.example.other"] });
        await userEvent.click(within(dialog).getByText("Notes"));
        expect(lastSaved().capture.excludedApps).toEqual(["com.example.other", "com.apple.notes"]);
        expect(screen.queryByRole("dialog", { name: "Exclude an app" })).not.toBeInTheDocument();
      });

      it("adds the lowercased app name when the app has no bundle id", async () => {
        const dialog = await openPicker();
        await userEvent.click(within(dialog).getByText("Legacy Tool"));
        expect(lastSaved().capture.excludedApps).toEqual(["legacy tool"]);
      });

      it("says nothing was recorded when there are no apps", async () => {
        mocked.appsSeen.mockResolvedValue([]);
        renderPage("capture");
        await userEvent.click(screen.getByRole("button", { name: /Add app/ }));
        expect(await screen.findByText("No apps recorded in the last 30 days.")).toBeInTheDocument();
      });
    });
  });
});

describe("Settings Capture page external sources", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists the names of the external sources", async () => {
    mocked.activitySources.mockResolvedValue(["planner", "calendar"]);
    renderPage("capture");
    expect(await screen.findByText("planner")).toBeInTheDocument();
    expect(screen.getByText("calendar")).toBeInTheDocument();
    expect(screen.queryByText("No external sources yet")).not.toBeInTheDocument();
  });

  it("says there are none yet and shows the CLI example when no source exists", async () => {
    mocked.activitySources.mockResolvedValue([]);
    renderPage("capture");
    expect(await screen.findByText("No external sources yet")).toBeInTheDocument();
    expect(screen.getByText(/timetracker activity add/)).toBeInTheDocument();
  });
});

describe("Settings Tracker page", () => {
  beforeEach(() => vi.clearAllMocks());

  it("saves the new away threshold when it is committed", async () => {
    renderPage("tracker");
    const field = screen.getByLabelText("Away after");
    expect(field).toHaveValue(5);
    await userEvent.clear(field);
    await userEvent.type(field, "10{Enter}");
    expect(lastSaved().tracker).toEqual({ ...settings().tracker, idleMinutes: 10 });
  });

  it("saves the new check interval when it is committed", async () => {
    renderPage("tracker");
    const field = screen.getByLabelText("Check every");
    await userEvent.clear(field);
    await userEvent.type(field, "4{Enter}");
    expect(lastSaved().tracker).toEqual({ ...settings().tracker, pollSeconds: 4 });
  });

  describe("away prompt", () => {
    const minutesField = () => screen.getByRole("combobox", { name: "For longer than" });
    const askSwitch = () => screen.getByRole("switch", { name: "Ask what I did after being away" });

    it("shows the prompt on with a threshold of 10 minutes", () => {
      renderPage("tracker");
      expect(askSwitch()).toBeChecked();
      expect(minutesField()).toHaveValue("10");
      expect(minutesField()).toBeEnabled();
    });

    it("offers 5 to 60 minutes", () => {
      renderPage("tracker");
      expect(within(minutesField()).getAllByRole("option").map((o) => o.textContent)).toEqual(["5 min", "10 min", "15 min", "20 min", "30 min", "45 min", "60 min"]);
    });

    it("keeps a hand-edited threshold selectable", () => {
      const base = settings();
      resetStore({ settings: { ...base, tracker: { ...base.tracker, awayPromptMinutes: 12 } }, dialog: { type: "settings", page: "tracker" } });
      installCommands();
      render(<SettingsDialog />);
      expect(minutesField()).toHaveValue("12");
    });

    it("saves the chosen threshold", async () => {
      renderPage("tracker");
      await userEvent.selectOptions(minutesField(), "30");
      expect(lastSaved().tracker).toEqual({ ...settings().tracker, awayPromptMinutes: 30 });
    });

    it("saves the prompt switched off and disables the threshold", async () => {
      renderPage("tracker");
      await userEvent.click(askSwitch());
      expect(lastSaved().tracker).toEqual({ ...settings().tracker, awayPromptEnabled: false });
      expect(minutesField()).toBeDisabled();
    });
  });

  it.each([
    ["Away after", "500"],
    ["Away after", "0"],
    ["Check every", "11"],
  ])("rejects %s = %s as out of range and restores the value", async (label, typed) => {
    renderPage("tracker");
    const field = screen.getByLabelText(label);
    const before = (field as HTMLInputElement).value;
    await userEvent.clear(field);
    await userEvent.type(field, `${typed}{Enter}`);
    expect(mocked.updateSettings).not.toHaveBeenCalled();
    await userEvent.tab();
    expect(field).toHaveValue(Number(before));
  });
});
