import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { resetStore } from "../../state/store";
import { at } from "../../test/dayFixtures";
import { storedSuggestion } from "../../test/ruleFixtures";
import { SuggestionBlock } from "./SuggestionBlock";

const y = (ms: number) => ms / 60_000;

describe("SuggestionBlock", () => {
  beforeEach(() => {
    resetStore();
  });

  it("names a finished suggestion by its start and end", () => {
    render(<SuggestionBlock suggestion={storedSuggestion({ start: at("09:00"), end: at("09:45") })} y={y} focused={false} />);
    const block = screen.getByRole("button", { name: "Suggested: Website 09:00–09:45, by cli" });
    expect(block).not.toHaveClass("running");
  });

  it("names a running suggestion by its start and marks it as running", () => {
    render(<SuggestionBlock suggestion={storedSuggestion({ running: true, start: at("09:00"), end: at("10:23") })} y={y} focused={false} />);
    expect(screen.getByRole("button", { name: "Suggested: Website since 09:00, by cli" })).toHaveClass("running");
  });

  it("shows No project for a running suggestion without one", () => {
    render(<SuggestionBlock suggestion={storedSuggestion({ running: true, projectId: null, projectName: null })} y={y} focused={false} />);
    expect(screen.getByRole("button", { name: "Suggested: No project since 09:00, by cli" })).toBeInTheDocument();
  });
});
