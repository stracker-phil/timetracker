import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { ReportCharts } from "../../api";
import { chartSlice } from "../../test/reportFixtures";
import { ReportChart } from "./ReportChart";

const charts: ReportCharts = {
  byClient: [chartSlice({ key: "c1", label: "Acme", roundedMs: 5_400_000, percent: 75, share: 0.75 }), chartSlice({ key: "c2", label: "Globex", roundedMs: 1_800_000, percent: 25, offset: 0.75, share: 0.25 })],
  byProject: [
    chartSlice({ key: "p1", label: "Portal", roundedMs: 3_600_000, percent: 50, share: 0.5 }),
    chartSlice({ key: "p2", label: "Docs", roundedMs: 1_800_000, percent: 25, offset: 0.5, share: 0.25 }),
    chartSlice({ key: "p3", label: "Site", roundedMs: 1_800_000, percent: 25, offset: 0.75, share: 0.25 }),
  ],
  byTag: [chartSlice({ key: "t1", label: "support", color: "red", roundedMs: 7_200_000 })],
};

const renderChart = (initial: "client" | "project" | "tag" = "client") => render(<ReportChart charts={charts} totalMs={7_200_000} initial={initial} />);

describe("ReportChart", () => {
  it("lists each slice of the initial grouping in the legend with its hours", () => {
    renderChart();
    const legend = within(screen.getByRole("list", { name: "By client legend" }));
    expect(legend.getAllByRole("listitem")).toHaveLength(2);
    expect(legend.getByText("Acme").parentElement).toHaveTextContent("1h 30m");
    expect(legend.getByText("Globex").parentElement).toHaveTextContent("30m");
  });

  it("shows the total and the number of slices in the center", () => {
    renderChart();
    const chart = within(screen.getByRole("img", { name: /^By client:/ }));
    expect(chart.getByText("2h 00m")).toBeInTheDocument();
    expect(chart.getByText("2 clients")).toBeInTheDocument();
  });

  it("describes the slices with their percentages for assistive technology", () => {
    renderChart();
    expect(screen.getByRole("img", { name: "By client: Acme 1h 30m (75 %), Globex 30m (25 %)" })).toBeInTheDocument();
  });

  it.each([
    ["project", "By project legend", ["Portal", "Docs", "Site"], "3 projects"],
    ["tag", "By tag legend", ["support"], "1 tag"],
  ])("switches to the slices by %s through the Chart by select", async (by, legendName, labels, caption) => {
    renderChart();
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Chart by" }), by);
    const legend = within(screen.getByRole("list", { name: legendName }));
    expect(legend.getAllByRole("listitem").map((item) => item.textContent)).toEqual(expect.arrayContaining(labels.map((label) => expect.stringContaining(label))));
    expect(screen.getByText(caption)).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "By client legend" })).not.toBeInTheDocument();
  });

  it("starts with the initial grouping", () => {
    renderChart("tag");
    expect(screen.getByRole("combobox", { name: "Chart by" })).toHaveValue("tag");
    expect(screen.getByRole("list", { name: "By tag legend" })).toBeInTheDocument();
  });

  it("shows an empty legend and a zero count without slices", () => {
    render(<ReportChart charts={{ byClient: [], byProject: [], byTag: [] }} totalMs={0} initial="client" />);
    expect(within(screen.getByRole("list", { name: "By client legend" })).queryAllByRole("listitem")).toHaveLength(0);
    expect(screen.getByText("0 clients")).toBeInTheDocument();
  });
});
