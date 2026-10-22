import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { reportResult } from "../../test/reportFixtures";
import { ReportTotals } from "./ReportTotals";

const totals = () => within(screen.getByLabelText("Totals"));

describe("ReportTotals", () => {
  it("shows the rounded total with the recording and project counts", () => {
    render(<ReportTotals result={reportResult({ totalMs: 4_500_000, totalRoundedMs: 4_500_000, recordingCount: 1234, projectCount: 3 })} />);
    expect(totals().getByText("Total").parentElement).toHaveTextContent("1h 15m");
    expect(totals().getByText("Recordings").parentElement).toHaveTextContent("1,234");
    expect(totals().getByText("Projects").parentElement).toHaveTextContent("3");
  });

  describe("untagged time", () => {
    it("is shown when recordings without a tag exist", () => {
      render(<ReportTotals result={reportResult({ untaggedRoundedMs: 1_800_000 })} />);
      expect(totals().getByText("Untagged").parentElement).toHaveTextContent("30m");
    });

    it("is left out when every recording has a tag", () => {
      render(<ReportTotals result={reportResult({ untaggedRoundedMs: 0 })} />);
      expect(totals().queryByText("Untagged")).not.toBeInTheDocument();
    });
  });

  describe("billable share", () => {
    it("shows the billable hours and the percentage", () => {
      render(<ReportTotals result={reportResult({ billable: { recordedMs: 28_800_000, billableMs: 23_040_000, percent: 80 } })} />);
      expect(totals().getByText("Billable").parentElement).toHaveTextContent("6h 24m · 80 %");
    });

    it("is left out when nothing is recorded", () => {
      render(<ReportTotals result={reportResult()} />);
      expect(totals().queryByText("Billable")).not.toBeInTheDocument();
    });
  });

  describe("total before rounding", () => {
    const filter = reportResult().filter;

    it("is shown next to the rounded total when rounding changed it", () => {
      render(<ReportTotals result={reportResult({ filter: { ...filter, rounding: "up30" }, totalMs: 2_700_000, totalRoundedMs: 3_600_000 })} />);
      expect(totals().getByText("Total").parentElement).toHaveTextContent("1h 00m");
      expect(totals().getByText("45m before rounding")).toBeInTheDocument();
    });

    it("is left out when no rounding is on", () => {
      render(<ReportTotals result={reportResult({ totalMs: 2_700_000, totalRoundedMs: 3_600_000 })} />);
      expect(totals().queryByText(/before rounding/)).not.toBeInTheDocument();
    });

    it("is left out when rounding left the total as it was", () => {
      render(<ReportTotals result={reportResult({ filter: { ...filter, rounding: "up15" } })} />);
      expect(totals().queryByText(/before rounding/)).not.toBeInTheDocument();
    });
  });
});
