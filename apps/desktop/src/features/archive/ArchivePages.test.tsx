import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BatchExtractPage, BrowserPage, CreatePage, ExtractPage, TaskCenter } from "./ArchivePages";
import { archiveClient } from "../../lib/archiveClient";
import type { ArchiveEntry, ArchiveSession, TaskSnapshot } from "../../contracts/archive";

describe("archive core flow controls", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: false });
  });

  it("submits a create task only once on consecutive clicks", async () => {
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: true });
    const resolveCreate = vi.fn<(task: TaskSnapshot) => void>();
    const suggest = vi.spyOn(archiveClient, "suggestCreateOutput").mockResolvedValue("D:\\资料\\项目.7z");
    const create = vi.spyOn(archiveClient, "create").mockImplementation(() => new Promise((resolve) => {
      resolveCreate.mockImplementationOnce(resolve);
    }));
    const onCreated = vi.fn();
    const onOpenTasks = vi.fn();
    render(<CreatePage initialInputs={["D:\\资料\\项目.txt"]} onBack={vi.fn()} onCreated={onCreated} onOpenTasks={onOpenTasks} />);

    const button = screen.getByRole("button", { name: "开始压缩" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(create).toHaveBeenCalledTimes(1);
    expect(button).toBeDisabled();

    resolveCreate({
      taskId: "task-create",
      operation: "create",
      status: "queued",
      displayName: "项目.7z",
      output: "D:\\资料\\项目.7z",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      warnings: [],
      retryable: false
    });
    await waitFor(() => expect(onOpenTasks).toHaveBeenCalledTimes(1));
    create.mockRestore();
    suggest.mockRestore();
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: false });
  });

  it("forwards one cancellation request from the task card", () => {
    const task: TaskSnapshot = {
      taskId: "task-running",
      operation: "create",
      status: "running",
      displayName: "项目.7z",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      warnings: [],
      retryable: false
    };
    const onCancel = vi.fn();
    render(<TaskCenter tasks={[task]} onBack={vi.fn()} onClear={vi.fn()} onCancel={onCancel} onRetry={vi.fn()} />);
    const cancel = screen.getByRole("button", { name: "取消任务" });
    expect(cancel).toHaveClass("qzip-button--danger");
    fireEvent.click(cancel);
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onCancel).toHaveBeenCalledWith("task-running");
  });

  it("requires and forwards a password for a wrong-password retry", () => {
    const task: TaskSnapshot = {
      taskId: "task-password",
      operation: "extract",
      status: "failed",
      displayName: "secret.7z",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      error: { code: "WRONG_PASSWORD", message: "密码错误", recoverable: true },
      warnings: [],
      retryable: true
    };
    const onRetry = vi.fn();
    render(<TaskCenter tasks={[task]} onBack={vi.fn()} onClear={vi.fn()} onCancel={vi.fn()} onRetry={onRetry} />);
    const retry = screen.getByRole("button", { name: "重新输入密码" });
    expect(retry).toHaveClass("qzip-button--warning");
    fireEvent.click(retry);
    const input = screen.getByLabelText("重试密码");
    fireEvent.change(input, { target: { value: "correct" } });
    fireEvent.click(screen.getByRole("button", { name: "确认重试" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
    expect(onRetry).toHaveBeenCalledWith("task-password", "correct");
  });

  it("formats Rust Unix-second completion timestamps as current dates", () => {
    const task: TaskSnapshot = {
      taskId: "task-completed",
      operation: "create",
      status: "completed",
      displayName: "项目.7z",
      createdAt: 1_700_000_000,
      updatedAt: 1_700_000_000,
      warnings: [],
      retryable: false
    };
    render(<TaskCenter tasks={[task]} onBack={vi.fn()} onClear={vi.fn()} onCancel={vi.fn()} onRetry={vi.fn()} />);
    expect(screen.getByText(/2023/).textContent).not.toContain("1970");
  });

  it("disables formats that the active backend cannot create", async () => {
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: true });
    vi.spyOn(archiveClient, "suggestCreateOutput").mockResolvedValue("D:\\资料\\项目.7z");
    vi.spyOn(archiveClient, "capabilities").mockResolvedValue({
      backendId: "test",
      version: "1",
      writableFormats: ["sevenZip"],
      readableFormats: ["sevenZip", "zip"],
      supportsPassword: true,
      supportsHeaderEncryption: true,
      supportsPartialExtract: true,
      supportsUpdate: false,
      supportsProgress: true,
      supportsCancellation: true
    });
    render(<CreatePage initialInputs={["D:\\资料\\项目.txt"]} onBack={vi.fn()} onCreated={vi.fn()} onOpenTasks={vi.fn()} />);
    await waitFor(() => expect(screen.getByRole("radio", { name: "ZIP" })).toBeDisabled());
    expect(screen.getByRole("radio", { name: "7Z" })).toBeEnabled();
  });

  it("appends the next archive-entry page without duplicating rows", async () => {
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: true });
    const first: ArchiveEntry = { path: "first.txt", displayName: "first.txt", size: 1, isDirectory: false, encrypted: false, isSymlink: false, isHardlink: false };
    const second: ArchiveEntry = { path: "second.txt", displayName: "second.txt", size: 1, isDirectory: false, encrypted: false, isSymlink: false, isHardlink: false };
    const session: ArchiveSession = { sessionId: "session", format: "sevenZip", compressedSize: 10, estimatedUncompressedSize: 20, entryCount: 2, encrypted: false, risks: [] };
    const entries = vi.spyOn(archiveClient, "entries")
      .mockResolvedValueOnce({ entries: [first], total: 2, nextOffset: 1 })
      .mockResolvedValueOnce({ entries: [first, second], total: 2, nextOffset: null });
    vi.spyOn(archiveClient, "recordPerformanceMarker").mockResolvedValue(undefined);
    render(<BrowserPage archive="D:\\large.7z" session={session} onBack={vi.fn()} onClose={vi.fn()} onExtract={vi.fn()} onCreated={vi.fn()} />);

    await screen.findByText("first.txt");
    fireEvent.click(screen.getByRole("button", { name: /加载更多/ }));
    await screen.findByText("second.txt");
    expect(screen.getAllByText("first.txt")).toHaveLength(1);
    expect(entries).toHaveBeenLastCalledWith("session", undefined, undefined, 1);
    await waitFor(() => expect(screen.queryByRole("button", { name: /加载更多/ })).not.toBeInTheDocument());
  });

  it("opens an archive file with the system-associated app on double-click", async () => {
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: true });
    const file: ArchiveEntry = { path: "资料/report.txt", displayName: "report.txt", size: 12, isDirectory: false, encrypted: false, isSymlink: false, isHardlink: false };
    const folder: ArchiveEntry = { path: "资料", displayName: "资料", size: 0, isDirectory: true, encrypted: false, isSymlink: false, isHardlink: false };
    const session: ArchiveSession = { sessionId: "session-open", format: "zip", compressedSize: 10, estimatedUncompressedSize: 12, entryCount: 2, encrypted: false, risks: [] };
    vi.spyOn(archiveClient, "entries").mockResolvedValue({ entries: [folder, file], total: 2, nextOffset: null });
    vi.spyOn(archiveClient, "recordPerformanceMarker").mockResolvedValue(undefined);
    const openEntry = vi.spyOn(archiveClient, "openEntry").mockResolvedValue(undefined);
    render(<BrowserPage archive="D:\\docs.zip" session={session} onBack={vi.fn()} onClose={vi.fn()} onExtract={vi.fn()} onCreated={vi.fn()} />);

    const row = await screen.findByRole("row", { name: /report\.txt/ });
    fireEvent.doubleClick(row);
    await waitFor(() => expect(openEntry).toHaveBeenCalledWith("session-open", "资料/report.txt"));
  });

  it("shows technical task details on demand", () => {
    const task: TaskSnapshot = {
      taskId: "task-detail",
      operation: "extract",
      status: "failed",
      displayName: "broken.zip",
      output: "D:\\output",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      error: { code: "CORRUPT_ARCHIVE", message: "损坏", recoverable: false },
      warnings: ["临时目录已清理"],
      retryable: false
    };
    render(<TaskCenter tasks={[task]} onBack={vi.fn()} onClear={vi.fn()} onCancel={vi.fn()} onRetry={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "查看详情" }));
    expect(screen.queryByText("task-detail")).not.toBeInTheDocument();
    expect(screen.getByText("解压")).toBeInTheDocument();
    expect(screen.getByText("CORRUPT_ARCHIVE")).toBeInTheDocument();
    expect(screen.getByText("临时目录已清理")).toBeInTheDocument();
  });

  it("shows the actual archive format on create task cards", () => {
    const task: TaskSnapshot = {
      taskId: "task-create-zip",
      operation: "create",
      status: "completed",
      displayName: "ui-smoke.zip",
      output: "D:\\output\\ui-smoke.zip",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      warnings: [],
      retryable: false
    };
    render(<TaskCenter tasks={[task]} onBack={vi.fn()} onClear={vi.fn()} onCancel={vi.fn()} onRetry={vi.fn()} />);
    expect(screen.getByRole("img", { name: "ZIP 文件图标" })).toHaveAttribute("src", "/file-types/zip.ico");
    expect(screen.queryByRole("img", { name: "7Z 文件图标" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "打开结果" })).toHaveClass("qzip-button--primary");
  });

  it("shows one list with active tasks first and highlights the focused task", () => {
    const active: TaskSnapshot = {
      taskId: "task-active",
      operation: "extract",
      status: "scanning",
      displayName: "incoming.zip",
      output: "D:\\output",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      progress: { phase: "scanning", elapsedSeconds: 3 },
      warnings: [],
      retryable: true
    };
    const completed: TaskSnapshot = {
      ...active,
      taskId: "task-completed",
      status: "completed",
      displayName: "done.zip",
      progress: { phase: "committing", percent: 100, elapsedSeconds: 4 }
    };
    render(<TaskCenter tasks={[completed, active]} focusTaskId="task-completed" onBack={vi.fn()} onClear={vi.fn()} onCancel={vi.fn()} onRetry={vi.fn()} />);
    expect(screen.getByText("共 2 个任务")).toBeInTheDocument();
    expect(screen.getAllByText("解压位置")).toHaveLength(2);
    expect(screen.getAllByText("D:\\output")).toHaveLength(2);
    expect(screen.getByText("正在检查压缩包")).toBeInTheDocument();
    expect(screen.queryByText("0%")).not.toBeInTheDocument();
    const cards = document.querySelectorAll(".qzip-task-card");
    expect(cards[0]).toHaveAttribute("data-status", "scanning");
    expect(document.querySelector('[data-focused="true"]')).toHaveAttribute("data-status", "completed");
    expect(screen.queryByText("进行中（1）")).not.toBeInTheDocument();
  });

  it("keeps the extraction result visible after a task is created", async () => {
    const task: TaskSnapshot = {
      taskId: "extract-visible",
      operation: "extract",
      status: "queued",
      displayName: "visible.zip",
      output: "D:\\visible",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      warnings: [],
      retryable: true
    };
    const onCreated = vi.fn();
    const onBack = vi.fn();
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: true });
    vi.spyOn(archiveClient, "extract").mockResolvedValue(task);
    const session: ArchiveSession = { sessionId: "visible", format: "zip", compressedSize: 10, estimatedUncompressedSize: 20, entryCount: 1, encrypted: false, risks: [] };
    render(<ExtractPage archive="D:\\visible.zip" session={session} onBack={onBack} onBrowse={vi.fn()} onCreated={onCreated} defaultConflictPolicy="rename" />);
    fireEvent.change(screen.getByLabelText("解压位置"), { target: { value: "D:\\visible" } });
    fireEvent.click(screen.getByRole("button", { name: "开始解压" }));

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(task));
    expect(onBack).not.toHaveBeenCalled();
  });

  it("starts safe batch extracts and reports archives that need individual handling", async () => {
    Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: true });
    const safeSession: ArchiveSession = { sessionId: "safe", format: "zip", compressedSize: 10, estimatedUncompressedSize: 20, entryCount: 1, encrypted: false, risks: [] };
    vi.spyOn(archiveClient, "prepare")
      .mockResolvedValueOnce(safeSession)
      .mockRejectedValueOnce({ code: "WRONG_PASSWORD", message: "需要密码" });
    vi.spyOn(archiveClient, "suggestExtractOutput").mockResolvedValue("D:\\safe");
    vi.spyOn(archiveClient, "extract").mockResolvedValue({
      taskId: "batch-safe",
      operation: "extract",
      status: "queued",
      displayName: "safe.zip",
      output: "D:\\safe",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      warnings: [],
      retryable: false
    });
    const onStarted = vi.fn();
    render(<BatchExtractPage archives={["D:\\safe.zip", "D:\\secret.7z"]} onBack={vi.fn()} onStarted={onStarted} />);
    fireEvent.click(screen.getByRole("button", { name: "开始批量解压" }));

    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    const [tasks, failures] = onStarted.mock.calls[0] as [TaskSnapshot[], { archive: string; message: string }[]];
    expect(tasks).toHaveLength(1);
    expect(failures).toEqual([{ archive: "D:\\secret.7z", message: "需要密码" }]);
  });
});
