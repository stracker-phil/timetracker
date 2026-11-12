import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import type { Settings } from "../../api";
import { installCommands } from "../../commands";
import { pageRows, settingsPages } from "../../registry/settingsPages";
import { resetStore } from "../../state/store";
import { renderWithClient, seedQuery, SETTINGS_KEY } from "../../test/queries";
import { settings } from "../../test/settings";
import { registerCoreSettingsPages } from "./pages";

registerCoreSettingsPages();

/** What shows once the page's own data has loaded. */
const LOADED: Record<string, string> = { data: "JSON backup", status: "Build" };

const labelOf = (element: Element): string | undefined => {
  const label = element.matches("section") ? element.querySelector("h3") : (element.querySelector(".settings-row-label") ?? element.querySelector("label"));
  return label?.textContent?.trim();
};

const variants: [string, Settings][] = [
  ["with every conditional row shown", settings()],
  ["with the summary lane off", settings({ timeline: { ...settings().timeline, summaryLane: false } })],
];

describe.each(variants)("The Settings search index %s", (_variant, current) => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    seedQuery(SETTINGS_KEY, current);
    installCommands();
  });

  describe.each(settingsPages().map((page) => page.id))("the %s page", (id) => {
    async function renderBody() {
      const page = settingsPages().find((p) => p.id === id)!;
      const { container } = renderWithClient(<page.Body s={current} set={() => {}} commit={async () => null} />);
      if (LOADED[id]) await screen.findByText(LOADED[id]);
      const anchored = [...container.querySelectorAll("[data-anchor]")].map((element) => ({ anchor: element.getAttribute("data-anchor")!, element }));
      return { page, rows: pageRows(page), anchored };
    }

    it("registers a row for every anchor it renders", async () => {
      const { rows, anchored } = await renderBody();
      const unlisted = anchored.map((a) => a.anchor).filter((anchor) => !rows.some((row) => row.id === anchor));
      expect(unlisted).toEqual([]);
    });

    it("renders each anchor once", async () => {
      const { anchored } = await renderBody();
      const anchors = anchored.map((a) => a.anchor);
      expect(anchors.filter((anchor, i) => anchors.indexOf(anchor) !== i)).toEqual([]);
    });

    it("renders the anchor of every row, or the one it is revealed under", async () => {
      const { rows, anchored } = await renderBody();
      const shown = new Set(anchored.map((a) => a.anchor));
      const unreachable = rows.filter((row) => !shown.has(row.id) && !(row.under && shown.has(row.under))).map((row) => row.id);
      expect(unreachable).toEqual([]);
    });

    it("titles each row as its anchored label reads", async () => {
      const { rows, anchored } = await renderBody();
      const mismatched = rows.flatMap((row) => {
        const found = anchored.find((a) => a.anchor === row.id);
        return found && labelOf(found.element) !== row.title ? [{ id: row.id, title: row.title, label: labelOf(found.element) }] : [];
      });
      expect(mismatched).toEqual([]);
    });
  });
});
