import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { at, DAY } from "../../test/dayFixtures";
import { storedSuggestion } from "../../test/ruleFixtures";
import { Toasts } from "../Toasts";
import { SuggestionCard } from "./SuggestionCard";

const mocked = vi.mocked(api, true);

function renderCard(patch: Parameters<typeof storedSuggestion>[0] = {}) {
  const suggestion = storedSuggestion({ note: "standup", start: at("09:00"), end: at("10:23"), ...patch });
  render(
    <>
      <SuggestionCard suggestion={suggestion} focused={false} />
      <Toasts />
    </>,
  );
  return suggestion;
}

describe("SuggestionCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ day: DAY });
  });

  it("names a finished suggestion by its start and end", () => {
    renderCard();
    const card = screen.getByRole("group", { name: "Suggested: standup, Website, 09:00–10:23, by cli" });
    expect(card).not.toHaveClass("running");
  });

  it("names a running suggestion by its start and marks it as running", () => {
    renderCard({ running: true });
    expect(screen.getByRole("group", { name: "Suggested: standup, Website, since 09:00, by cli" })).toHaveClass("running");
  });

  it.each([
    ["with a project", {}],
    ["without a project", { projectId: null, projectName: null }],
  ])("offers no approve, pick or dismiss button on a running suggestion %s", (_, patch) => {
    renderCard({ running: true, ...patch });
    expect(screen.queryByRole("button", { name: "Approve" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Pick a project" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Dismiss" })).not.toBeInTheDocument();
  });

  it("dismisses a finished suggestion", async () => {
    const suggestion = renderCard();
    mocked.suggestionsDismiss.mockResolvedValueOnce(1);
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(await screen.findByText("Dismissed 1 suggestion")).toBeInTheDocument();
    expect(mocked.suggestionsDismiss).toHaveBeenCalledWith(DAY, [suggestion.id]);
  });

  it("approving a finished suggestion toasts what was recorded", async () => {
    renderCard();
    await userEvent.click(screen.getByRole("button", { name: "Approve" }));
    expect(await screen.findByText("Recorded 45m for Website")).toBeInTheDocument();
  });
});
