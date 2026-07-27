import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { useAppearanceStore } from "../stores/appearance";

describe("App appearance controls", () => {
  afterEach(() => {
    useAppearanceStore.setState({ mode: "light", accent: "mint" });
    document.documentElement.dataset.mode = "light";
    document.documentElement.dataset.accent = "mint";
  });

  it("updates the document theme from the settings popover", async () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    fireEvent.click(screen.getByRole("radio", { name: "暗夜" }));
    fireEvent.click(screen.getByRole("radio", { name: "海洋蓝" }));

    await waitFor(() => {
      expect(document.documentElement.dataset.mode).toBe("dark");
      expect(document.documentElement.dataset.accent).toBe("ocean");
    });
  });
});
