import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore } from "../state/store";
import { AppIcon } from "./AppIcon";

const mocked = vi.mocked(api, true);

const SAFARI = "data:image/png;base64,SAFARI";
const MAIL = "data:image/png;base64,MAIL";

describe("AppIcon", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    mocked.appIcons.mockResolvedValue({});
  });

  it("shows the icon image when Rust resolves one for the bundle id", async () => {
    mocked.appIcons.mockResolvedValue({ "com.apple.Safari": SAFARI });
    const { container } = render(<AppIcon app="Safari" bundleId="com.apple.Safari" />);

    await waitFor(() => expect(container.querySelector("img")).toHaveAttribute("src", SAFARI));
    expect(screen.queryByText("S")).not.toBeInTheDocument();
  });

  it("shows the uppercase initial while the icon is not resolved and when none exists", async () => {
    const { container } = render(<AppIcon app="geany" bundleId="org.geany" />);

    expect(screen.getByText("G")).toBeInTheDocument();
    await waitFor(() => expect(mocked.appIcons).toHaveBeenCalled());
    expect(screen.getByText("G")).toBeInTheDocument();
    expect(container.querySelector("img")).toBeNull();
  });

  it("shows the initial without asking Rust when the bundle id is null", () => {
    const { container } = render(<AppIcon app="  calendar sync" bundleId={null} />);

    expect(screen.getByText("C")).toBeInTheDocument();
    expect(container.querySelector("img")).toBeNull();
    expect(mocked.appIcons).not.toHaveBeenCalled();
  });

  it("shows the initial when loading the icons fails", async () => {
    mocked.appIcons.mockRejectedValue({ kind: "io", message: "boom", code: null });
    const { container } = render(<AppIcon app="Mail" bundleId="com.apple.mail" />);

    await waitFor(() => expect(mocked.appIcons).toHaveBeenCalled());
    expect(screen.getByText("M")).toBeInTheDocument();
    expect(container.querySelector("img")).toBeNull();
  });

  it("gives each icon its own image when several are rendered together", async () => {
    mocked.appIcons.mockResolvedValue({ "com.apple.Safari": SAFARI, "com.apple.mail": MAIL });
    const { container } = render(
      <>
        <AppIcon app="Safari" bundleId="com.apple.Safari" />
        <AppIcon app="Mail" bundleId="com.apple.mail" size={16} />
        <AppIcon app="Safari" bundleId="com.apple.Safari" size={16} />
        <AppIcon app="Notes" bundleId="com.apple.Notes" />
      </>,
    );

    await waitFor(() => expect(container.querySelectorAll("img")).toHaveLength(3));
    const sources = Array.from(container.querySelectorAll("img")).map((img) => img.getAttribute("src"));
    expect(sources).toEqual([SAFARI, MAIL, SAFARI]);
    expect(screen.getByText("N")).toBeInTheDocument();
  });
});
