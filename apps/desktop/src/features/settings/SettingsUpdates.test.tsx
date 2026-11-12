import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { SettingsPage } from "./SettingsPage";
import type { AppUpdates } from "./useAppUpdates";
import { defaultAppSettings } from "../../contracts/settings";

it.each(["idle", "downloading", "ready"] as const)("shares the update dialog between settings sections in the %s state", (phase) => {
  const check = vi.fn().mockResolvedValue(undefined);
  const updates: AppUpdates = {
    result: null, phase, progress: null,
    downloaded: phase === "ready" ? { token: "verified", version: "v2.1.0" } : null,
    error: null, cancelled: false, cancelling: false,
    check, download: vi.fn(), cancel: vi.fn(), install: vi.fn(), openRelease: vi.fn()
  };
  const { container } = render(<SettingsPage settings={defaultAppSettings} updates={updates} onBack={vi.fn()} onChanged={vi.fn()} onToast={vi.fn()} />);
  const dialog = container.querySelector("dialog")!;
  dialog.showModal = vi.fn(() => dialog.setAttribute("open", ""));
  dialog.close = vi.fn(() => dialog.removeAttribute("open"));

  fireEvent.click(screen.getByRole("button", { name: "系统与更新" }));
  expect(container.querySelectorAll(".qzip-update-card")).toHaveLength(1);
  expect(container.querySelector(".qzip-settings-card-stack .qzip-update-card")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  expect(dialog).toHaveAttribute("open");
  expect(within(dialog).getByRole("region", { name: "应用更新" })).toBeInTheDocument();
  expect(check).toHaveBeenCalledTimes(phase === "idle" ? 1 : 0);
  fireEvent.click(within(dialog).getByRole("button", { name: "关闭更新弹窗" }));
  expect(dialog).not.toHaveAttribute("open");

  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  expect(container.querySelector("dialog")).toBe(dialog);
  expect(dialog).toHaveAttribute("open");
  expect(check).toHaveBeenCalledTimes(phase === "idle" ? 2 : 0);
  if (phase === "downloading") expect(within(dialog).getByRole("button", { name: "取消下载" })).toBeInTheDocument();
  if (phase === "ready") expect(within(dialog).getByText(/下载完成，SHA-256 校验通过/)).toBeInTheDocument();
});
