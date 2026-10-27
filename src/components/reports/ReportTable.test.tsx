import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { resetStore } from "../../state/store";
import { at } from "../../test/dayFixtures";
import { reportClient, reportProject, reportRow } from "../../test/reportFixtures";
import { Toasts } from "../Toasts";
import { ReportTable } from "./ReportTable";

const clients = [
  reportClient({
    key: "c1",
    label: "Acme",
    color: "green",
    roundedMs: 8_100_000,
    projects: [
      reportProject({
        id: "portal",
        name: "Portal",
        label: "Acme / Portal",
        roundedMs: 6_300_000,
        notes: { text: "Fixed the header\nReview\nDeploy\nPlan", count: 4 },
        rows: [
          reportRow({ id: "r1", note: "Fixed the header", tag: "support" }),
          reportRow({ id: "r2", start: at("14:00"), end: at("15:00"), durationMs: 3_600_000, roundedMs: 3_600_000 }),
        ],
      }),
      reportProject({ id: "docs", name: "Docs", label: "Acme / Docs", billable: true, roundedMs: 1_800_000, rows: [reportRow({ id: "r3" })] }),
    ],
  }),
  reportClient({ roundedMs: 2_700_000, projects: [reportProject({ id: "misc", name: "Misc", label: "Misc", roundedMs: 2_700_000, rows: [reportRow({ id: "r4" })] })] }),
];

const toggle = (name: string) => screen.getByRole("button", { name: new RegExp(`^${name}\\s+(billable\\s+)?\\d+ recordings?`) });

function renderTable() {
  render(
    <>
      <ReportTable clients={clients} />
      <Toasts />
    </>,
  );
}

describe("ReportTable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it("shows the hours of each client and project", () => {
    renderTable();
    expect(screen.getByRole("row", { name: /^Acme 2h 15m/ })).toBeInTheDocument();
    expect(screen.getByRole("row", { name: /^No client 45m/ })).toBeInTheDocument();
    expect(screen.getByRole("row", { name: /^Portal/ })).toHaveTextContent("1h 45m");
    expect(screen.getByRole("row", { name: /^Docs/ })).toHaveTextContent("30m");
  });

  it("counts the recordings of a project and badges a billable one", () => {
    renderTable();
    expect(toggle("Portal")).toHaveTextContent("2 recordings");
    expect(toggle("Misc")).toHaveTextContent("1 recording");
    expect(within(screen.getByRole("row", { name: /^Docs/ })).getByText("billable")).toBeInTheDocument();
    expect(within(screen.getByRole("row", { name: /^Portal/ })).queryByText("billable")).not.toBeInTheDocument();
  });

  describe("project recordings", () => {
    it("are hidden until the project is unfolded", () => {
      renderTable();
      expect(toggle("Portal")).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByText("Fixed the header")).not.toBeInTheDocument();
    });

    it("show the times, note, tag and hours of each recording when the project is unfolded", async () => {
      renderTable();
      await userEvent.click(toggle("Portal"));
      expect(toggle("Portal")).toHaveAttribute("aria-expanded", "true");
      const first = screen.getByRole("row", { name: /Fixed the header/ });
      expect(within(first).getByText("09:00–09:45")).toBeInTheDocument();
      expect(within(first).getByText("Fixed the header")).toBeInTheDocument();
      expect(within(first).getByText("support")).toBeInTheDocument();
      expect(first).toHaveTextContent("45m");
      expect(screen.getByText(/14:00–15:00/).closest("tr")).toHaveTextContent("1h 00m");
      expect(screen.queryAllByText(/09:00–09:45/)).toHaveLength(1);
    });

    it("leave out the day when the report covers a single day", async () => {
      renderTable();
      await userEvent.click(toggle("Portal"));
      expect(screen.queryByText("Wed, Oct 21")).not.toBeInTheDocument();
    });

    it("lead with the day when the report covers several days", async () => {
      render(<ReportTable clients={clients} multiDay />);
      await userEvent.click(toggle("Portal"));
      const first = screen.getByRole("row", { name: /Fixed the header/ });
      expect(within(first).getByText("Wed, Oct 21")).toBeInTheDocument();
      expect(within(first).getByText("09:00–09:45")).toBeInTheDocument();
    });

    it("has no column headers", () => {
      renderTable();
      expect(screen.queryByRole("columnheader")).not.toBeInTheDocument();
    });

    it("show every span of a recording in one row", async () => {
      const spans = [
        { start: at("14:25"), end: at("15:00") },
        { start: at("16:05"), end: at("20:15") },
      ];
      render(<ReportTable clients={[reportClient({ projects: [reportProject({ rows: [reportRow({ spans, start: spans[0].start, end: spans[1].end })] })] })]} />);
      await userEvent.click(toggle("Website"));
      expect(screen.getByText(/14:25–15:00, 16:05–20:15/)).toBeInTheDocument();
    });

    it("fold again when the project is clicked twice", async () => {
      renderTable();
      await userEvent.click(toggle("Portal"));
      await userEvent.click(toggle("Portal"));
      expect(toggle("Portal")).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByText("Fixed the header")).not.toBeInTheDocument();
    });
  });

  describe("Expand all", () => {
    it("unfolds every project and turns into Collapse all, which folds them again", async () => {
      renderTable();
      await userEvent.click(screen.getByRole("button", { name: "Expand all" }));
      for (const name of ["Portal", "Docs", "Misc"]) expect(toggle(name)).toHaveAttribute("aria-expanded", "true");
      await userEvent.click(screen.getByRole("button", { name: "Collapse all" }));
      for (const name of ["Portal", "Docs", "Misc"]) expect(toggle(name)).toHaveAttribute("aria-expanded", "false");
      expect(screen.getByRole("button", { name: "Expand all" })).toBeInTheDocument();
    });

    it("unfolds the rest when only some projects are open", async () => {
      renderTable();
      await userEvent.click(toggle("Docs"));
      await userEvent.click(screen.getByRole("button", { name: "Expand all" }));
      expect(toggle("Portal")).toHaveAttribute("aria-expanded", "true");
      expect(toggle("Docs")).toHaveAttribute("aria-expanded", "true");
    });
  });

  describe("Copy notes", () => {
    it("writes the notes of the project to the clipboard and confirms with their count", async () => {
      renderTable();
      await userEvent.click(screen.getByRole("button", { name: "Copy notes of Acme / Portal" }));
      expect(await screen.findByText("Copied 4 notes for Acme / Portal")).toBeInTheDocument();
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("Fixed the header\nReview\nDeploy\nPlan");
    });

    it("is disabled for a project without notes", async () => {
      renderTable();
      const button = screen.getByRole("button", { name: "Copy notes of Acme / Docs" });
      expect(button).toBeDisabled();
      await userEvent.click(button);
      expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    });
  });
});
