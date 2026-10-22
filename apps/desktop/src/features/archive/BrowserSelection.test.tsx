import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { BrowserPage } from "./BrowserPage";
import { ExtractPage } from "./ArchivePages";
import { archiveClient } from "../../lib/archiveClient";
import type { ArchiveEntry, ArchiveSession } from "../../contracts/archive";

const session: ArchiveSession = { sessionId: "selection", format: "zip", compressedSize: 10, estimatedUncompressedSize: 20, entryCount: 3, encrypted: false, risks: [] };
const files: ArchiveEntry[] = ["a.txt", "b.txt", "c.txt"].map(path => ({ path, displayName: path, size: 1, isDirectory: false, encrypted: false, isSymlink: false, isHardlink: false }));
function setup() {
  Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: true });
  vi.spyOn(archiveClient, "recordPerformanceMarker").mockResolvedValue(undefined);
  vi.spyOn(archiveClient, "systemFileIcons").mockResolvedValue({});
}
afterEach(() => { vi.restoreAllMocks(); Object.defineProperty(archiveClient, "isTauri", { configurable: true, value: false }); });

it("supports single, Ctrl and Shift selection, keyboard opening and clearing", async () => {
  setup();
  vi.spyOn(archiveClient, "entries").mockResolvedValue({ entries: files, total: 3 });
  const open = vi.spyOn(archiveClient, "openEntry").mockResolvedValue(undefined);
  const extract = vi.fn();
  render(<BrowserPage archive="D:\\docs.zip" session={session} onBack={vi.fn()} onClose={vi.fn()} onExtract={extract} onCreated={vi.fn()} />);
  const a = await screen.findByRole("row", { name: /a.txt/ });
  const b = screen.getByRole("row", { name: /b.txt/ });
  const c = screen.getByRole("row", { name: /c.txt/ });
  fireEvent.click(a); fireEvent.click(c, { ctrlKey: true });
  expect(a).toHaveAttribute("aria-selected", "true"); expect(c).toHaveAttribute("aria-selected", "true");
  fireEvent.click(b);
  expect(a).toHaveAttribute("aria-selected", "false"); expect(b).toHaveAttribute("aria-selected", "true");
  fireEvent.click(c, { shiftKey: true });
  fireEvent.click(screen.getByRole("button", { name: "解压选中 (2)" }));
  expect(extract).toHaveBeenCalledWith(["b.txt", "c.txt"]);
  fireEvent.keyDown(b, { key: "Enter" });
  expect(open).toHaveBeenCalledWith("selection", "b.txt");
  fireEvent.keyDown(screen.getByRole("grid"), { key: "Escape" });
  expect(screen.getByText("已选择 0 项")).toBeInTheDocument();
  fireEvent.keyDown(screen.getByRole("grid"), { key: "f", ctrlKey: true });
  expect(screen.getByLabelText("搜索当前文件夹")).toHaveFocus();
});

it("selects results beyond the loaded page and includes them when extracting", async () => {
  setup();
  vi.spyOn(archiveClient, "entries").mockResolvedValueOnce({ entries: [files[0]!], total: 3, nextOffset: 1 }).mockResolvedValueOnce({ entries: files.slice(1), total: 3 });
  const extract = vi.fn();
  render(<BrowserPage archive="D:\\docs.zip" session={session} onBack={vi.fn()} onClose={vi.fn()} onExtract={extract} onCreated={vi.fn()} />);
  await screen.findByText("a.txt");
  fireEvent.keyDown(screen.getByRole("grid"), { key: "a", ctrlKey: true });
  fireEvent.click(await screen.findByRole("button", { name: "解压选中 (3)" }));
  expect(extract).toHaveBeenCalledWith(["a.txt", "b.txt", "c.txt"]);
  expect(screen.queryByRole("button", { name: /加载更多/ })).not.toBeInTheDocument();
});

it("respects extraction preferences without overwriting a user-entered destination", async () => {
  setup();
  const suggest = vi.spyOn(archiveClient, "suggestExtractOutput").mockResolvedValue("D:\\建议位置");
  const props = { archive: "D:\\docs.zip", session, onBack: vi.fn(), onBrowse: vi.fn(), onCreated: vi.fn() };
  const view = render(<ExtractPage {...props} extractToNamedFolder={false} avoidDuplicateRootFolder={false} />);
  await waitFor(() => expect(screen.getByLabelText("解压位置")).toHaveValue("D:\\建议位置"));
  expect(suggest).toHaveBeenCalledWith("D:\\docs.zip", false, "selection", false);
  fireEvent.change(screen.getByLabelText("解压位置"), { target: { value: "D:\\手动选择" } });
  view.rerender(<ExtractPage {...props} extractToNamedFolder avoidDuplicateRootFolder />);
  expect(screen.getByLabelText("解压位置")).toHaveValue("D:\\手动选择");
  expect(suggest).toHaveBeenCalledTimes(1);
});
