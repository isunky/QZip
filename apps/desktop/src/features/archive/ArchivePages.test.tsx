import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CreatePage, TaskCenter } from "./ArchivePages";
import { archiveClient } from "../../lib/archiveClient";
import type { TaskSnapshot } from "../../contracts/archive";

describe("archive core flow controls", () => {
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
    fireEvent.click(screen.getByRole("button", { name: "取消任务" }));
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
    fireEvent.click(screen.getByRole("button", { name: "重新输入密码" }));
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
    expect(screen.getByText(/完成时间：/).textContent).not.toContain("1970");
  });
});
