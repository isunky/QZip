import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Header } from "./Header";

vi.mock("../lib/platform", () => ({
  usePlatform: () => ({ os: "macos", arch: "aarch64", nativeWindowControls: true })
}));

describe("macOS titlebar", () => {
  it("reserves native traffic light space without Windows controls", () => {
    const { container } = render(<Header iconSrc="/icon.png" onHomeClick={() => undefined} onTasksClick={() => undefined} onSettingsClick={() => undefined} />);
    expect(container.querySelector(".qzip-window-controls")).toBeNull();
    expect(container.querySelector("header")?.style.paddingLeft).toBe("84px");
    expect(screen.getByRole("button", { name: /设置|Settings/ })).toBeTruthy();
  });
});
