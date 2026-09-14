import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { UpdateCard } from "./UpdateCard";
import { useAppUpdates } from "./useAppUpdates";
import { settingsClient } from "../../lib/settingsClient";
import type { DownloadedUpdate, UpdateCheckResult, UpdateDownloadProgress } from "../../contracts/settings";

const release: UpdateCheckResult = {
  configured: true, status: "update_available", currentVersion: "1.1.2", latestVersion: "1.2.0",
  releaseUrl: "https://github.com/isunky/QZip/releases/tag/v1.2.0", releaseName: "QZip v1.2.0",
  publishedAt: "2026-09-13T16:58:10Z", releaseTag: "v1.2.0", downloadAvailable: true, downloadSize: 1024,
  releaseNotes: "## 改进\n- 更清晰的文件列表\n\n> 未签名版本可能显示 SmartScreen 提示。\n\n![tracking](https://example.com/pixel.png)"
};
function Harness() { return <UpdateCard updates={useAppUpdates(false)} currentVersion="1.1.2" />; }
afterEach(() => vi.restoreAllMocks());

it("shows release notes and downloads the displayed version before allowing installation", async () => {
  vi.spyOn(settingsClient, "checkForUpdates").mockResolvedValue(release);
  let complete!: (value: DownloadedUpdate) => void;
  let progress!: (value: UpdateDownloadProgress) => void;
  const download = vi.spyOn(settingsClient, "downloadUpdate").mockImplementation((_tag, callback) => {
    progress = callback;
    return new Promise(resolve => { complete = resolve; });
  });
  const install = vi.spyOn(settingsClient, "installUpdate").mockRejectedValue({ code: "UPDATE_TASKS_ACTIVE", message: "请等待当前压缩任务完成后再安装更新。" });
  render(<Harness />);
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("更清晰的文件列表")).toBeInTheDocument();
  expect(screen.getByText(/未签名版本可能/)).toBeInTheDocument();
  expect(screen.queryByRole("img", { name: "tracking" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "下载更新" }));
  expect(download).toHaveBeenCalledWith("v1.2.0", expect.any(Function));
  act(() => progress({ phase: "downloading", downloaded: 512, total: 1024 }));
  expect(screen.getByRole("progressbar")).toHaveAttribute("value", "50");
  await act(async () => complete({ token: "verified-token", version: "v1.2.0" }));
  fireEvent.click(screen.getByRole("button", { name: "安装更新" }));
  expect(install).toHaveBeenCalledWith("verified-token");
  expect(await screen.findByRole("alert")).toHaveTextContent("请等待当前压缩任务");
});

it("cancels a pending download and permits retry without showing a failure", async () => {
  vi.spyOn(settingsClient, "checkForUpdates").mockResolvedValue(release);
  let reject!: (reason: unknown) => void;
  vi.spyOn(settingsClient, "downloadUpdate").mockImplementation(() => new Promise((_resolve, fail) => { reject = fail; }));
  const cancel = vi.spyOn(settingsClient, "cancelDownload").mockImplementation(async () => { reject({ code: "UPDATE_CANCELLED", message: "下载已取消。" }); });
  render(<Harness />);
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  fireEvent.click(await screen.findByRole("button", { name: "下载更新" }));
  fireEvent.click(screen.getByRole("button", { name: "取消下载" }));
  expect(await screen.findByRole("button", { name: "重新下载" })).toBeEnabled();
  expect(cancel).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

it("never offers installation when checksum verification fails", async () => {
  vi.spyOn(settingsClient, "checkForUpdates").mockResolvedValue(release);
  vi.spyOn(settingsClient, "downloadUpdate").mockRejectedValue({ code: "UPDATE_CHECKSUM_MISMATCH", message: "安装包校验失败，请重新下载。" });
  render(<Harness />);
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  fireEvent.click(await screen.findByRole("button", { name: "下载更新" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("安装包校验失败");
  expect(screen.queryByRole("button", { name: "安装更新" })).not.toBeInTheDocument();
});
