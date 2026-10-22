import { useEffect, useRef, useState } from "react";
import {
  AddCircleRegular,
  ArchiveRegular,
  ArrowClockwiseRegular,
  ArrowDownloadRegular,
  ArrowLeftRegular,
  ArrowSortDownRegular,
  ArrowSortUpRegular,
  ChevronDownRegular,
  ChevronRightRegular,
  DismissRegular,
  DocumentRegular,
  FolderAddRegular,
  FolderRegular,
  HomeRegular,
  LockClosedRegular,
  MoreHorizontalRegular,
  SearchRegular,
  ShieldCheckmarkRegular
} from "@fluentui/react-icons";
import { Card, Input } from "@qzip/ui";
import type { ArchiveEntry, ArchiveSession, TaskSnapshot } from "../../contracts/archive";
import { archiveClient } from "../../lib/archiveClient";
import { useI18n } from "../../lib/i18n";
import { Empty } from "./ArchiveShared";
import {
  demoEntries,
  errorMessage,
  fileExtension,
  fileName,
  formatBytes,
  formatLabels,
  formatType,
  makeDemoTask,
  type EntrySortKey,
  type SortDirection
} from "./archiveUtils";

export function BrowserPage({
  archive,
  session,
  onBack,
  onClose,
  onExtract,
  onCreated
}: {
  archive: string;
  session: ArchiveSession;
  onBack: () => void;
  onClose: () => void;
  onExtract: (selectedEntries?: string[]) => void;
  onCreated: (task: TaskSnapshot) => void;
}) {
  const { locale, text } = useI18n();
  const [search, setSearch] = useState("");
  const [directory, setDirectory] = useState("");
  const [sortKey, setSortKey] = useState<EntrySortKey>("name");
  const [sortDirection, setSortDirection] = useState<SortDirection>("ascending");
  const [entries, setEntries] = useState<ArchiveEntry[]>(archiveClient.isTauri ? [] : demoEntries);
  const [loading, setLoading] = useState(archiveClient.isTauri);
  const [loadingMore, setLoadingMore] = useState(false);
  const [total, setTotal] = useState(archiveClient.isTauri ? 0 : demoEntries.length);
  const [nextOffset, setNextOffset] = useState<number | undefined>();
  const [loadError, setLoadError] = useState<string | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  const [openingEntry, setOpeningEntry] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const anchorRef = useRef<string | null>(null);
  const [selectingAll, setSelectingAll] = useState(false);
  const [contextEntry, setContextEntry] = useState<ArchiveEntry | null>(null);
  const [menuPoint, setMenuPoint] = useState({ x: 0, y: 0 });
  const menuOrigin = useRef<HTMLButtonElement | null>(null);
  const searchRef = useRef<HTMLDivElement>(null);
  const [propertiesOpen, setPropertiesOpen] = useState(false);
  const [systemFileIcons, setSystemFileIcons] = useState<Record<string, string>>({});
  const requestedIconExtensionsRef = useRef(new Set<string>());
  const reportedSessionRef = useRef<string | null>(null);
  const listGenerationRef = useRef(0);
  const selectionScope = useRef("");

  useEffect(() => {
    if (!contextEntry) return;
    const dismiss = (event: PointerEvent) => {
      if (!(event.target instanceof Element) || !event.target.closest(".qzip-entry-menu")) setContextEntry(null);
    };
    document.addEventListener("pointerdown", dismiss);
    return () => document.removeEventListener("pointerdown", dismiss);
  }, [contextEntry]);

  useEffect(() => {
    if (!archiveClient.isTauri) return;
    const generation = ++listGenerationRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (cancelled) return;
      setLoading(true);
      setLoadError(null);
      const scope = `${session.sessionId}\0${directory}\0${search}`;
      if (scope !== selectionScope.current) { setSelected(new Set()); anchorRef.current = null; selectionScope.current = scope; }
      setEntries([]);
      setTotal(0);
      setNextOffset(undefined);
      void archiveClient.entries(session.sessionId, directory || undefined, search || undefined, 0, sortKey, sortDirection)
        .then((page) => {
          if (cancelled || generation !== listGenerationRef.current) return;
          setEntries(page.entries);
          setTotal(page.total);
          setNextOffset(page.nextOffset ?? undefined);
        })
        .catch((reason) => {
          if (!cancelled && generation === listGenerationRef.current) setLoadError(errorMessage(reason));
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    }, search ? 220 : 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [directory, search, session.sessionId, sortDirection, sortKey]);
  useEffect(() => {
    if (!archiveClient.isTauri || loading || loadError || reportedSessionRef.current === session.sessionId) return;
    reportedSessionRef.current = session.sessionId;
    void archiveClient.recordPerformanceMarker("archive-list-first-page").catch(() => undefined);
  }, [loadError, loading, session.sessionId]);
  useEffect(() => {
    if (!archiveClient.isTauri) return;
    const extensions = [...new Set(entries
      .filter((entry) => !entry.isDirectory)
      .map((entry) => fileExtension(entry.displayName))
      .filter(Boolean))]
      .filter((extension) => !requestedIconExtensionsRef.current.has(extension));
    if (!extensions.length) return;
    extensions.forEach((extension) => requestedIconExtensionsRef.current.add(extension));
    let cancelled = false;
    void archiveClient.systemFileIcons(extensions)
      .then((icons) => {
        if (!cancelled) setSystemFileIcons((current) => ({ ...current, ...icons }));
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [entries]);

  const visibleEntries = !archiveClient.isTauri && directory ? [] : entries;
  const shown = archiveClient.isTauri ? visibleEntries : visibleEntries.filter((entry) => entry.displayName.toLowerCase().includes(search.toLowerCase()));
  const breadcrumbs = directory.split("/").filter(Boolean);
  const ratio = session.estimatedUncompressedSize
    ? Math.max(0, Math.min(100, 100 - (session.compressedSize / session.estimatedUncompressedSize) * 100))
    : 0;

  function selectEntry(path: string, modifiers: { ctrlKey?: boolean; metaKey?: boolean; shiftKey?: boolean } = {}) {
    setContextEntry(null);
    setSelected((current) => {
      if (modifiers.shiftKey && anchorRef.current) {
        const anchor = shown.findIndex(entry => entry.path === anchorRef.current);
        const end = shown.findIndex(entry => entry.path === path);
        if (anchor >= 0 && end >= 0) {
          const next = modifiers.ctrlKey || modifiers.metaKey ? new Set(current) : new Set<string>();
          shown.slice(Math.min(anchor, end), Math.max(anchor, end) + 1).forEach(entry => next.add(entry.path));
          return next;
        }
      }
      if (!modifiers.ctrlKey && !modifiers.metaKey) return new Set([path]);
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
    if (!modifiers.shiftKey) anchorRef.current = path;
  }
  async function selectAll() {
    if (loading || selectingAll || loadingMore) return;
    const generation = listGenerationRef.current;
    setSelectingAll(true);
    try {
      let all = [...shown];
      let offset = nextOffset;
      while (archiveClient.isTauri && offset != null) {
        const page = await archiveClient.entries(session.sessionId, directory || undefined, search || undefined, offset, sortKey, sortDirection);
        if (generation !== listGenerationRef.current) return;
        all = [...new Map([...all, ...page.entries].map(entry => [entry.path, entry])).values()];
        const next = page.nextOffset ?? undefined;
        if (next != null && next <= offset) throw new Error(text("列表未能继续加载，请重试。", "The list did not advance. Please retry."));
        offset = next;
      }
      if (generation !== listGenerationRef.current) return;
      if (archiveClient.isTauri) { setEntries(all); setNextOffset(undefined); }
      setSelected(new Set(all.map(entry => entry.path)));
    } catch (reason) { if (generation === listGenerationRef.current) setLoadError(errorMessage(reason)); }
    finally { setSelectingAll(false); }
  }
  function goUp() {
    if (search) setSearch("");
    else if (breadcrumbs.length) navigateBreadcrumb(breadcrumbs.length - 2);
  }
  function handleKeys(event: React.KeyboardEvent<HTMLElement>) {
    const input = event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault(); searchRef.current?.querySelector("input")?.focus(); return;
    }
    if (event.key === "Escape") { if (contextEntry) { setContextEntry(null); menuOrigin.current?.focus(); } else if (search) setSearch(""); else setSelected(new Set()); return; }
    if (input) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") { event.preventDefault(); void selectAll(); }
    if (event.key === "Backspace" || (event.altKey && event.key === "ArrowUp")) { event.preventDefault(); goUp(); }
  }
  function changeSort(nextKey: EntrySortKey) {
    setSortDirection((current) => sortKey === nextKey && current === "ascending" ? "descending" : "ascending");
    setSortKey(nextKey);
  }
  function openDirectory(entry: ArchiveEntry) {
    if (!entry.isDirectory) return;
    setDirectory(entry.path.endsWith("/") ? entry.path : `${entry.path}/`);
    setSelected(new Set());
    setSearch("");
  }
  async function openEntry(entry: ArchiveEntry) {
    if (entry.isDirectory) {
      openDirectory(entry);
      return;
    }
    if (!archiveClient.isTauri || openingEntry) return;
    setOpeningEntry(entry.path);
    setOpenError(null);
    try {
      await archiveClient.openEntry(session.sessionId, entry.path);
    } catch (reason) {
      setOpenError(errorMessage(reason));
    } finally {
      setOpeningEntry(null);
    }
  }
  function navigateBreadcrumb(index: number) {
    setDirectory(index < 0 ? "" : `${breadcrumbs.slice(0, index + 1).join("/")}/`);
    setSelected(new Set());
  }
  async function loadMore() {
    if (!archiveClient.isTauri || nextOffset == null || loadingMore || selectingAll) return;
    const generation = listGenerationRef.current;
    setLoadingMore(true);
    setLoadError(null);
    try {
      const page = await archiveClient.entries(session.sessionId, directory || undefined, search || undefined, nextOffset, sortKey, sortDirection);
      if (generation !== listGenerationRef.current) return;
      setEntries((current) => {
        const known = new Set(current.map((entry) => entry.path));
        return [...current, ...page.entries.filter((entry) => !known.has(entry.path))];
      });
      setTotal(page.total);
      setNextOffset(page.nextOffset ?? undefined);
    } catch (reason) {
      if (generation === listGenerationRef.current) setLoadError(errorMessage(reason));
    } finally {
      setLoadingMore(false);
    }
  }
  async function addToArchive(folder: boolean) {
    const inputs = archiveClient.isTauri
      ? folder
        ? [await archiveClient.pickInputFolder()].filter((value): value is string => Boolean(value))
        : await archiveClient.pickInputPaths(false)
      : [folder ? text("D:\\新增文件夹", "D:\\New folder") : text("D:\\新增文件.txt", "D:\\New file.txt")];
    if (!inputs.length) return;
    const task = archiveClient.isTauri
      ? await archiveClient.update({ archive, inputs })
      : makeDemoTask("update", fileName(archive), archive);
    onCreated(task);
  }
  async function testArchive() {
    const task = archiveClient.isTauri
      ? await archiveClient.test(archive)
      : makeDemoTask("test", fileName(archive), archive);
    onCreated(task);
  }

  return (
    <section className="qzip-browser-page" onKeyDown={handleKeys}>
      <header className="qzip-browser-page__topbar">
        <button type="button" className="qzip-square-action" aria-label={text("返回首页", "Back to home")} onClick={onBack}>
          <ArrowLeftRegular fontSize={24} />
        </button>
        <div className="qzip-browser-page__title-icon"><ArchiveRegular fontSize={28} /><span>{formatLabels[session.format]}</span></div>
        <h1>{fileName(archive)}</h1>
        <div className="qzip-browser-actions">
          <button type="button" onClick={() => void addToArchive(false).catch(reason => setOpenError(errorMessage(reason)))}><AddCircleRegular fontSize={22} /> {text("添加", "Add")}</button>
          <button type="button" disabled={selectingAll || loading} onClick={() => onExtract([...selected])}><ArrowDownloadRegular fontSize={22} /> {selected.size ? text(`解压选中 (${selected.size})`, `Extract selected (${selected.size})`) : text("解压全部", "Extract all")}</button>
          <button type="button" onClick={() => void testArchive().catch(reason => setOpenError(errorMessage(reason)))}><ShieldCheckmarkRegular fontSize={22} /> {text("测试", "Test")}</button>
          <button type="button" aria-expanded={propertiesOpen} onClick={() => setPropertiesOpen((value) => !value)}><MoreHorizontalRegular fontSize={22} /> {text("更多", "More")} <ChevronDownRegular fontSize={16} /></button>
        </div>
        {propertiesOpen ? (
          <div className="qzip-browser-properties">
            <strong>{text("压缩包属性", "Archive properties")}</strong>
            <span>{formatLabels[session.format]} · {text(`${session.entryCount} 项`, `${session.entryCount} items`)}</span>
            <button type="button" onClick={() => void addToArchive(true)}><FolderAddRegular fontSize={18} /> {text("添加文件夹", "Add folder")}</button>
            <button type="button" onClick={onClose}><DismissRegular fontSize={18} /> {text("关闭压缩包", "Close archive")}</button>
          </div>
        ) : null}
      </header>

      <Card className="qzip-browser-card">
        <div className="qzip-browser-toolbar">
          <nav className="qzip-breadcrumb" aria-label={text("压缩包路径", "Archive path")}>
            <button type="button" aria-label={text("根目录", "Root folder")} onClick={() => navigateBreadcrumb(-1)}><HomeRegular fontSize={21} /></button>
            {breadcrumbs.map((part, index) => (
              <span key={`${part}-${index}`}>
                <ChevronRightRegular fontSize={17} />
                <button type="button" onClick={() => navigateBreadcrumb(index)}>{part}</button>
              </span>
            ))}
          </nav>
          <div className="qzip-browser-search" ref={searchRef}><Input aria-label={text("搜索当前文件夹", "Search current folder")} placeholder={text("搜索当前文件夹", "Search current folder")} value={search} onChange={(event) => setSearch(event.target.value)} trailing={<SearchRegular fontSize={20} />} /></div>
        </div>
        <div className="qzip-selection-toolbar">
          <span role="status">{text(`已选择 ${selected.size} 项`, `${selected.size} selected`)}</span>
          <button type="button" disabled={loading || selectingAll || loadingMore} onClick={() => void selectAll()}>{selectingAll ? text("正在全选…", "Selecting…") : text("全选", "Select all")}</button>
          <button type="button" disabled={!selected.size} onClick={() => setSelected(new Set())}>{text("取消选择", "Clear selection")}</button>
          {directory ? <button type="button" onClick={goUp}>{text("上一级", "Up one level")}</button> : null}
        </div>
        {contextEntry ? <div className="qzip-entry-menu" style={{ left: menuPoint.x, top: menuPoint.y }} role="menu" aria-label={text("文件操作", "File actions")}>
          <button role="menuitem" autoFocus onClick={() => { void openEntry(contextEntry); setContextEntry(null); }}>{text("打开", "Open")}</button>
          <button role="menuitem" onClick={() => { onExtract(selected.has(contextEntry.path) ? [...selected] : [contextEntry.path]); setContextEntry(null); }}>{text("解压选中", "Extract selected")}</button>
          <button role="menuitem" onClick={() => setContextEntry(null)}>{text("关闭菜单", "Close menu")}</button>
        </div> : null}
        <div className="qzip-entry-table" role="grid" aria-label={text("压缩包文件列表", "Archive file list")} aria-multiselectable="true" aria-busy={loading || selectingAll}>
          <div className="qzip-entry-table__head" role="row">
            {([
              ["name", text("名称", "Name")],
              ["size", text("大小", "Size")],
              ["type", text("类型", "Type")],
              ["modified", text("修改时间", "Modified")]
            ] as const).map(([key, label]) => (
              <span key={key} role="columnheader" aria-sort={sortKey === key ? sortDirection : "none"}>
                <button type="button" onClick={() => changeSort(key)}>
                  {label}
                  {sortKey === key
                    ? sortDirection === "ascending"
                      ? <ArrowSortUpRegular fontSize={15} />
                      : <ArrowSortDownRegular fontSize={15} />
                    : null}
                </button>
              </span>
            ))}
          </div>
          {loading ? <Empty icon={<ArrowClockwiseRegular fontSize={34} className="qzip-spin" />} text={text("正在读取压缩包目录…", "Reading archive contents…")} /> : shown.map((entry) => (
            <button
              className="qzip-entry-row"
              key={entry.path}
              role="row"
              aria-selected={selected.has(entry.path)}
              onClick={(event) => selectEntry(entry.path, event)}
              onDoubleClick={() => void openEntry(entry)}
              onContextMenu={event => { event.preventDefault(); if (!selected.has(entry.path)) selectEntry(entry.path); menuOrigin.current = event.currentTarget; setMenuPoint({ x: Math.max(0, Math.min(event.clientX, window.innerWidth - 180)), y: Math.max(0, Math.min(event.clientY, window.innerHeight - 150)) }); setContextEntry(entry); }}
              onKeyDown={event => {
                if (event.key === "Enter") { event.preventDefault(); void openEntry(entry); }
                else if (event.key === " ") { event.preventDefault(); selectEntry(entry.path, { ctrlKey: true }); }
                else if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) { event.preventDefault(); if (!selected.has(entry.path)) selectEntry(entry.path); const rect = event.currentTarget.getBoundingClientRect(); menuOrigin.current = event.currentTarget; setMenuPoint({ x: Math.max(0, Math.min(rect.left + 30, window.innerWidth - 180)), y: Math.max(0, Math.min(rect.bottom, window.innerHeight - 150)) }); setContextEntry(entry); }
                else if (!event.altKey && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
                  event.preventDefault();
                  const index = shown.findIndex(item => item.path === entry.path);
                  const next = shown[index + (event.key === "ArrowDown" ? 1 : -1)];
                  if (next) { if (!event.ctrlKey && !event.metaKey) selectEntry(next.path, event); const rows = event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>("button.qzip-entry-row"); rows?.[shown.indexOf(next)]?.focus(); }
                }
              }}
              data-selected={selected.has(entry.path)}
              disabled={openingEntry === entry.path}
              title={entry.isDirectory ? text("双击打开文件夹", "Double-click to open folder") : text("双击使用默认应用打开", "Double-click to open with the default app")}
            >
              <span role="gridcell">
                {openingEntry === entry.path
                  ? <ArrowClockwiseRegular fontSize={23} className="qzip-spin" />
                  : entry.isDirectory
                    ? <FolderRegular fontSize={23} />
                    : systemFileIcons[fileExtension(entry.displayName)]
                      ? <img className="qzip-entry-row__file-icon" src={systemFileIcons[fileExtension(entry.displayName)]} alt="" aria-hidden="true" />
                      : <DocumentRegular fontSize={23} />}
                {entry.displayName}{entry.encrypted ? <LockClosedRegular fontSize={14} /> : null}
              </span>
              <span role="gridcell">{entry.isDirectory ? "—" : formatBytes(entry.size)}</span>
              <span role="gridcell">{formatType(entry, locale)}</span>
              <span role="gridcell">{entry.modifiedAt?.replace("T", " ").slice(0, 16) ?? "—"}</span>
            </button>
          ))}
          {loadError ? <div className="qzip-browser-load-error">{text("读取列表失败：", "Could not read the list: ")}{loadError}</div> : null}
          {openError ? <div className="qzip-browser-load-error">{text("无法打开文件：", "Could not open the file: ")}{openError}</div> : null}
          {nextOffset != null && !loading ? <button type="button" className="qzip-load-more" disabled={loadingMore} onClick={() => void loadMore()}>{loadingMore ? text("正在加载…", "Loading…") : text(`加载更多（已显示 ${entries.length}/${total}）`, `Load more (${entries.length}/${total} shown)`)}</button> : null}
          {!shown.length && !loading ? <Empty icon={<SearchRegular fontSize={34} />} text={directory ? text("此文件夹为空", "This folder is empty") : text("没有匹配的文件", "No matching files")} /> : null}
        </div>
        <footer className="qzip-browser-footer">
          <span>{directory || search ? text(`当前结果 ${total} 项`, `${total} results`) : text(`共 ${session.entryCount} 项`, `${session.entryCount} items total`)} · {text(`已显示 ${entries.length}`, `${entries.length} shown`)}</span>
          <span>{text("原始大小：", "Original size: ")}{formatBytes(session.estimatedUncompressedSize)}</span>
          <span>{text("压缩后大小：", "Compressed size: ")}{formatBytes(session.compressedSize)}</span>
          <strong>{text("压缩率", "Compression ratio")} {ratio.toFixed(1)}%</strong>
        </footer>
      </Card>
    </section>
  );
}
