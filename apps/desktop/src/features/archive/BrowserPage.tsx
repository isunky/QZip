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
import {
  demoEntries,
  Empty,
  errorMessage,
  fileExtension,
  fileName,
  formatBytes,
  formatLabels,
  formatType,
  makeDemoTask,
  type EntrySortKey,
  type SortDirection
} from "./ArchiveShared";

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
  const [propertiesOpen, setPropertiesOpen] = useState(false);
  const [systemFileIcons, setSystemFileIcons] = useState<Record<string, string>>({});
  const requestedIconExtensionsRef = useRef(new Set<string>());
  const reportedSessionRef = useRef<string | null>(null);
  const listGenerationRef = useRef(0);

  useEffect(() => {
    if (!archiveClient.isTauri) return;
    const generation = ++listGenerationRef.current;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (cancelled) return;
      setLoading(true);
      setLoadError(null);
      setSelected(new Set());
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
    void archiveClient.recordPerformanceMarker("archive-list-first-page");
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

  function toggleSelected(path: string) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
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
    if (!archiveClient.isTauri || nextOffset == null || loadingMore) return;
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
    <section className="qzip-browser-page">
      <header className="qzip-browser-page__topbar">
        <button type="button" className="qzip-square-action" aria-label={text("返回快速解压", "Back to quick extract")} onClick={onBack}>
          <ArrowLeftRegular fontSize={24} />
        </button>
        <div className="qzip-browser-page__title-icon"><ArchiveRegular fontSize={28} /><span>{formatLabels[session.format]}</span></div>
        <h1>{fileName(archive)}</h1>
        <div className="qzip-browser-actions">
          <button type="button" onClick={() => void addToArchive(false)}><AddCircleRegular fontSize={22} /> {text("添加", "Add")} <ChevronDownRegular fontSize={16} /></button>
          <button type="button" onClick={() => onExtract([...selected])}><ArrowDownloadRegular fontSize={22} /> {text("解压", "Extract")} <ChevronDownRegular fontSize={16} /></button>
          <button type="button" onClick={() => void testArchive()}><ShieldCheckmarkRegular fontSize={22} /> {text("测试", "Test")} <ChevronDownRegular fontSize={16} /></button>
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
          <Input aria-label={text("搜索压缩包内容", "Search archive contents")} placeholder={text("搜索", "Search")} value={search} onChange={(event) => setSearch(event.target.value)} trailing={<SearchRegular fontSize={20} />} />
        </div>
        <div className="qzip-entry-table" role="table">
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
              onClick={() => toggleSelected(entry.path)}
              onDoubleClick={() => void openEntry(entry)}
              data-selected={selected.has(entry.path)}
              disabled={openingEntry === entry.path}
              title={entry.isDirectory ? text("双击打开文件夹", "Double-click to open folder") : text("双击使用默认应用打开", "Double-click to open with the default app")}
            >
              <span>
                {openingEntry === entry.path
                  ? <ArrowClockwiseRegular fontSize={23} className="qzip-spin" />
                  : entry.isDirectory
                    ? <FolderRegular fontSize={23} />
                    : systemFileIcons[fileExtension(entry.displayName)]
                      ? <img className="qzip-entry-row__file-icon" src={systemFileIcons[fileExtension(entry.displayName)]} alt="" aria-hidden="true" />
                      : <DocumentRegular fontSize={23} />}
                {entry.displayName}{entry.encrypted ? <LockClosedRegular fontSize={14} /> : null}
              </span>
              <span>{entry.isDirectory ? "—" : formatBytes(entry.size)}</span>
              <span>{formatType(entry, locale)}</span>
              <span>{entry.modifiedAt?.replace("T", " ").slice(0, 16) ?? "—"}</span>
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
