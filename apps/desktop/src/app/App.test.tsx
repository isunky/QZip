import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { useAppearanceStore } from "../stores/appearance";
import { settingsClient } from "../lib/settingsClient";

describe("App appearance controls", () => {
  afterEach(async () => {
    await settingsClient.reset();
    useAppearanceStore.setState({ mode: "light", accent: "mint" });
    document.documentElement.dataset.mode = "light";
    document.documentElement.dataset.accent = "mint";
  });

  it("updates the document theme from the full settings page", async () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    fireEvent.click(screen.getByRole("radio", { name: "暗夜" }));
    fireEvent.click(screen.getByRole("radio", { name: "海洋" }));

    await waitFor(() => {
      expect(document.documentElement.dataset.mode).toBe("dark");
      expect(document.documentElement.dataset.accent).toBe("ocean");
    });
  });

  it("switches the interface and brand name to English", async () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    fireEvent.click(screen.getByRole("radio", { name: "English" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Settings" })).toBeInTheDocument();
      expect(screen.getByText("QZip", { selector: ".qzip-brand__name" })).toBeInTheDocument();
      expect(document.documentElement.lang).toBe("en-US");
      expect(document.title).toBe("QZip");
    });
  });
});
