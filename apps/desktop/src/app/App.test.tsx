import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { useAppearanceStore } from "../stores/appearance";
import { settingsClient } from "../lib/settingsClient";
import { archiveClient } from "../lib/archiveClient";
import { defaultAppSettings } from "../contracts/settings";
import type { TaskEvent } from "../contracts/archive";

describe("App appearance controls", () => {
  afterEach(async () => {
    vi.restoreAllMocks();
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: false });
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

  it("keeps system settings and the about page visually separate", async () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    fireEvent.click(screen.getByRole("button", { name: "系统与更新" }));
    expect(screen.getByRole("heading", { name: "系统与更新" })).toBeInTheDocument();
    expect(screen.getByText("系统集成")).toBeInTheDocument();
    expect(screen.queryByText("使用情况遥测")).not.toBeInTheDocument();
    expect(screen.queryByText("轻压 · QZip")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "关于" }));
    expect(screen.getByRole("heading", { name: "关于轻压" })).toBeInTheDocument();
    expect(screen.getByText("轻压 · QZip")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "轻压应用图标" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /GitHub 项目/ })).toBeInTheDocument();
    expect(screen.queryByText("系统集成")).not.toBeInTheDocument();
  });

  it("opens directly into browsing and retains selection, scroll and extraction drafts across navigation", async () => {
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: true });
    const session = { sessionId: "session-switch", format: "zip" as const, compressedSize: 10, estimatedUncompressedSize: 20, entryCount: 1, encrypted: false, risks: [] };
    vi.spyOn(archiveClient, "tasks").mockResolvedValue([]);
    vi.spyOn(archiveClient, "onTaskEvent").mockResolvedValue(() => undefined);
    vi.spyOn(archiveClient, "onLaunchRequest").mockResolvedValue(() => undefined);
    vi.spyOn(archiveClient, "takeInitialLaunchRequest").mockResolvedValue(null);
    vi.spyOn(archiveClient, "takePendingShellRequest").mockResolvedValue(null);
    vi.spyOn(archiveClient, "recordPerformanceMarker").mockResolvedValue(undefined);
    vi.spyOn(archiveClient, "pickInputPaths").mockResolvedValue(["D:\\sample.zip"]);
    vi.spyOn(archiveClient, "prepare").mockResolvedValue(session);
    vi.spyOn(archiveClient, "suggestExtractOutput").mockResolvedValue("D:\\sample");
    vi.spyOn(archiveClient, "entries").mockResolvedValue({ entries: [{ path: "report.txt", displayName: "report.txt", size: 20, isDirectory: false, encrypted: false, isSymlink: false, isHardlink: false }], total: 1 });
    vi.spyOn(archiveClient, "systemFileIcons").mockResolvedValue({});
    const close = vi.spyOn(archiveClient, "close").mockResolvedValue(undefined);

    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "打开压缩包" }));
    const row = await screen.findByRole("row", { name: /report.txt/ });
    fireEvent.click(row);
    const list = screen.getByRole("grid");
    list.scrollTop = 120;

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    expect(close).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "返回压缩包" }));
    expect(screen.getByRole("grid")).toBe(list);
    expect(list.scrollTop).toBe(120);
    expect(row).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("button", { name: "解压选中 (1)" }));
    await waitFor(() => expect(screen.getByLabelText("解压位置")).toHaveValue("D:\\sample"));
    fireEvent.change(screen.getByLabelText("解压位置"), { target: { value: "D:\\自选目录" } });
    fireEvent.click(screen.getByRole("button", { name: "任务" }));
    fireEvent.click(screen.getByRole("button", { name: "返回压缩包" }));
    expect(screen.getByLabelText("解压位置")).toHaveValue("D:\\自选目录");
    fireEvent.click(screen.getByRole("button", { name: "查看压缩包内容" }));
    fireEvent.click(screen.getByRole("button", { name: "更多" }));
    fireEvent.click(screen.getByRole("button", { name: "关闭压缩包" }));
    expect(close).toHaveBeenCalledTimes(1);
    expect(close).toHaveBeenCalledWith("session-switch");
    expect(screen.queryByRole("button", { name: "返回压缩包" })).not.toBeInTheDocument();
  });

  it("opens an extraction output once on completion, but not for resync or failed tasks", async () => {
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: true });
    vi.spyOn(settingsClient, "get").mockResolvedValue({ ...defaultAppSettings, openFolderAfterExtract: true });
    vi.spyOn(archiveClient, "tasks").mockResolvedValue([]);
    let onEvent!: (event: TaskEvent) => void;
    vi.spyOn(archiveClient, "onTaskEvent").mockImplementation(async handler => { onEvent = handler; return () => undefined; });
    vi.spyOn(archiveClient, "onLaunchRequest").mockResolvedValue(() => undefined);
    vi.spyOn(archiveClient, "takeInitialLaunchRequest").mockResolvedValue(null);
    vi.spyOn(archiveClient, "takePendingShellRequest").mockResolvedValue(null);
    vi.spyOn(archiveClient, "recordPerformanceMarker").mockResolvedValue(undefined);
    const open = vi.spyOn(archiveClient, "open").mockResolvedValue(undefined);
    render(<App />);
    await act(async () => undefined);
    const event: TaskEvent = { eventType: "task.completed", task: { taskId: "extract-once", operation: "extract", status: "completed", displayName: "资料.zip", output: "D:\\资料", createdAt: 1, updatedAt: 2, warnings: [], retryable: false } };
    act(() => { onEvent({ ...event, eventType: "task.resync" }); onEvent({ ...event, task: { ...event.task, status: "failed" } }); });
    expect(open).not.toHaveBeenCalled();
    act(() => { onEvent(event); onEvent(event); });
    expect(open).toHaveBeenCalledTimes(1);
    expect(open).toHaveBeenCalledWith("D:\\资料");
  });
});
