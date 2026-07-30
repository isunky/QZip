import { useCallback, useEffect, useRef, useState } from "react";
import {
  AddCircleRegular,
  ArchiveRegular,
  ArrowClockwiseRegular,
  ArrowDownloadRegular,
  ArrowLeftRegular,
  CheckmarkCircleRegular,
  ChevronDownRegular,
  ChevronRightRegular,
  DeleteRegular,
  DismissRegular,
  DocumentAddRegular,
  DocumentRegular,
  ErrorCircleRegular,
  EyeRegular,
  FolderAddRegular,
  FolderOpenRegular,
  FolderRegular,
  HomeRegular,
  LockClosedRegular,
  MoreHorizontalRegular,
  OpenRegular,
  PauseRegular,
  SearchRegular,
  ShieldCheckmarkRegular,
  WarningRegular
} from "@fluentui/react-icons";
import { Button, Card, Input, Progress, SegmentedControl } from "@qzip/ui";
import type {
  ArchiveEntry,
  ArchiveFormat,
  ArchiveRisk,
  ArchiveSession,
  CompressionProfile,
  ConflictPolicy,
  CreateTaskRequest,
  TaskSnapshot
} from "../../contracts/archive";
import { archiveClient } from "../../lib/archiveClient";
import { joinOutputPath, splitOutputPath, suggestCreateOutputLocally } from "./archivePath";

type Page = "home" | "create" | "extract" | "browser" | "tasks";

const primaryFormatOptions = [
  { value: "sevenZip", label: "7Z" },
  { value: "zip", label: "ZIP" },
  { value: "tar", label: "TAR" }
] as const;
const advancedFormatOptions = [
  { value: "tarGz", label: "TAR.GZ" },
  { value: "tarXz", label: "TAR.XZ" }
] as const;
const profiles = [
  { value: "fast", label: "快速" },
  { value: "balanced", label: "均衡" },
  { value: "small", label: "更小" }
] as const;
const conflictOptions = [
  { value: "rename", label: "重命名" },
  { value: "overwrite", label: "覆盖" },
  { value: "skip", label: "跳过" }
] as const;

const demoEntries: ArchiveEntry[] = [
  { path: "设计资料/", displayName: "设计资料", size: 0, isDirectory: true, modifiedAt: "2024-05-20T10:24:00", encrypted: false, isSymlink: false, isHardlink: false },
  { path: "项目文档/", displayName: "项目文档", size: 0, isDirectory: true, modifiedAt: "2024-05-18T09:15:00", encrypted: false, isSymlink: false, isHardlink: false },
  { path: "需求说明.md", displayName: "需求说明.md", size: 159_744, compressedSize: 58_400, isDirectory: false, modifiedAt: "2024-05-20T10:24:00", encrypted: false, isSymlink: false, isHardlink: false },
  { path: "项目预算.xlsx", displayName: "项目预算.xlsx", size: 49_869, compressedSize: 31_220, isDirectory: false, modifiedAt: "2024-05-19T16:42:00", encrypted: false, isSymlink: false, isHardlink: false },
  { path: "原型文件.fig", displayName: "原型文件.fig", size: 12_897_485, compressedSize: 4_287_800, isDirectory: false, modifiedAt: "2024-05-18T14:08:00", encrypted: false, isSymlink: false, isHardlink: false }
];

const formatLabels: Record<ArchiveFormat, string> = {
  sevenZip: "7Z",
  zip: "ZIP",
  tar: "TAR",
  tarGz: "TAR.GZ",
  tarXz: "TAR.XZ",
  rar: "RAR",
  gz: "GZ",
  xz: "XZ",
  bz2: "BZ2",
  iso: "ISO",
  cab: "CAB",
  wim: "WIM",
  unknown: "未知"
};

function formatBytes(value: number) {
  if (!value) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${(value / 1024 ** index).toFixed(index ? 1 : 0)} ${units[index]}`;
}

function fileName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

function formatType(entry: ArchiveEntry) {
  if (entry.isDirectory) return "文件夹";
  const extension = entry.displayName.split(".").pop()?.toUpperCase();
  return extension ? `${extension} 文件` : "文件";
}

function formatElapsed(seconds = 0) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = Math.floor(seconds % 60);
  return [hours, minutes, rest].map((value) => String(value).padStart(2, "0")).join(":");
}

function formatTaskTimestamp(value: number) {
  // Rust task-runtime timestamps are Unix seconds; demo/test tasks use JS
  // milliseconds. Accept both while persisted task history is migrated.
  const milliseconds = value < 100_000_000_000 ? value * 1000 : value;
  const date = new Date(milliseconds);
  return Number.isNaN(date.getTime()) ? "—" : date.toLocaleString();
}

function makeDemoTask(operation: "create" | "extract" | "test" | "update", name: string, output?: string): TaskSnapshot {
  return {
    taskId: crypto.randomUUID(),
    operation,
    status: "queued",
    displayName: name,
    output,
    createdAt: Date.now(),
    updatedAt: Date.now(),
    warnings: [],
    retryable: true
  };
}

function RiskNotice({ risks, accepted, onAccepted }: {
  risks: ArchiveRisk[];
  accepted: boolean;
  onAccepted: (value: boolean) => void;
}) {
  if (!risks.length) return null;
  const mayContinue = risks.some((risk) => risk.overridable);
  return (
    <aside className="qzip-risk-notice">
      <WarningRegular fontSize={22} />
      <div>
        <strong>安全检查提示</strong>
        <p>{risks.map((risk) => risk.message).join("；")}</p>
        {mayContinue ? (
          <label>
            <input type="checkbox" checked={accepted} onChange={(event) => onAccepted(event.target.checked)} />
            我已了解本次风险并继续
          </label>
        ) : null}
      </div>
    </aside>
  );
}

export function CreatePage({
  onBack,
  onCreated,
  onOpenTasks,
  defaultFormat = "sevenZip",
  defaultProfile = "balanced",
  defaultTestAfterCreate = true,
  initialInputs = []
}: {
  onBack: () => void;
  onCreated: (task: TaskSnapshot) => void;
  onOpenTasks: () => void;
  defaultFormat?: ArchiveFormat;
  defaultProfile?: CompressionProfile;
  defaultTestAfterCreate?: boolean;
  initialInputs?: string[];
}) {
  const [inputs, setInputs] = useState<string[]>(() => [...new Set(initialInputs)]);
  const [format, setFormat] = useState<ArchiveFormat>(defaultFormat);
  const [profile, setProfile] = useState<CompressionProfile>(defaultProfile);
  const initialOutput = splitOutputPath(
    suggestCreateOutputLocally(initialInputs, defaultFormat)
      ?? (archiveClient.isTauri ? "" : "D:\\示例\\项目资料.7z")
  );
  const [directory, setDirectory] = useState(initialOutput.directory);
  const [name, setName] = useState(initialOutput.name);
  const [password, setPassword] = useState("");
  const [passwordOpen, setPasswordOpen] = useState(false);
  const [moreOpen, setMoreOpen] = useState(false);
  const [testing, setTesting] = useState(defaultTestAfterCreate);
  const [encryptHeaders, setEncryptHeaders] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submittingRef = useRef(false);
  const suggestionGeneration = useRef(0);
  const initialSuggestion = useRef({ inputs: [...new Set(initialInputs)], format: defaultFormat });

  const applySuggestedOutput = useCallback((path: string) => {
    const next = splitOutputPath(path);
    setDirectory(next.directory);
    setName(next.name);
  }, []);
  const suggestOutput = useCallback((nextInputs: string[], nextFormat: ArchiveFormat) => {
    const local = suggestCreateOutputLocally(nextInputs, nextFormat);
    if (local) applySuggestedOutput(local);
    if (!nextInputs.length || !archiveClient.isTauri) return;
    const generation = ++suggestionGeneration.current;
    void archiveClient.suggestCreateOutput(nextInputs, nextFormat)
      .then((path) => {
        if (generation === suggestionGeneration.current) applySuggestedOutput(path);
      })
      .catch(() => undefined);
  }, [applySuggestedOutput]);
  const replaceInputs = (nextInputs: string[]) => {
    const unique = [...new Set(nextInputs)];
    setInputs(unique);
    suggestOutput(unique, format);
  };
  const selectFormat = (nextFormat: ArchiveFormat) => {
    setFormat(nextFormat);
    suggestOutput(inputs, nextFormat);
  };
  const addFiles = async () => {
    if (!archiveClient.isTauri) {
      replaceInputs(["D:\\项目资料"]);
      return;
    }
    const picked = await archiveClient.pickInputPaths(false);
    replaceInputs([...inputs, ...picked]);
  };
  const addFolder = async () => {
    if (!archiveClient.isTauri) {
      setInputs(["D:\\项目资料"]);
      return;
    }
    const picked = await archiveClient.pickInputFolder();
    if (picked) replaceInputs([...inputs, picked]);
  };
  const pickOutputFolder = async () => {
    if (!archiveClient.isTauri) {
      setDirectory("D:\\桌面");
      return;
    }
    const picked = await archiveClient.pickInputFolder();
    if (picked) setDirectory(picked);
  };

  useEffect(() => {
    const { inputs: initialInputsForSuggestion, format: initialFormat } = initialSuggestion.current;
    if (!initialInputsForSuggestion.length || !archiveClient.isTauri) return;
    let cancelled = false;
    void archiveClient.suggestCreateOutput(initialInputsForSuggestion, initialFormat)
      .then((path) => {
        if (!cancelled) applySuggestedOutput(path);
      })
      .catch(() => {
        // The local suggestion keeps the create flow usable if IPC path
        // suggestion is temporarily unavailable.
      });
    return () => {
      cancelled = true;
    };
  }, [applySuggestedOutput]);

  const start = async () => {
    if (busy || submittingRef.current) return;
    if (!inputs.length || !directory.trim() || !name.trim()) {
      setError("请先选择要压缩的文件，并填写保存位置和文件名。");
      return;
    }
    if (format === "tar" && password) {
      setError("TAR 格式不支持密码，请改用 7Z 或 ZIP。");
      return;
    }
    submittingRef.current = true;
    setBusy(true);
    setError(null);
    const output = joinOutputPath(directory, name);
    try {
      const request: CreateTaskRequest = {
        inputs,
        output,
        format,
        profile,
        password: password || undefined,
        encryptHeaders: Boolean(password) && format === "sevenZip" && encryptHeaders,
        testAfterCreate: testing,
        deleteSourcesAfterSuccess: false
      };
      onCreated(archiveClient.isTauri ? await archiveClient.create(request) : makeDemoTask("create", name || "项目资料.7z", output));
      onOpenTasks();
    } catch (reason) {
      setError(String(reason));
    } finally {
      submittingRef.current = false;
      setBusy(false);
    }
  };

  const summaryName = inputs.length === 1 ? fileName(inputs[0] ?? "") : `${inputs.length} 个对象`;
  return (
    <DetailWorkspace title="创建压缩包" onBack={onBack} className="qzip-create-page">
      <section className="qzip-selection-summary">
        <div className="qzip-selection-summary__icon"><FolderRegular fontSize={38} /></div>
        <div>
          <strong>{inputs.length ? summaryName : "选择要压缩的文件"}</strong>
          <span>{inputs.length ? `${inputs.length} 个已选对象` : "可添加文件或整个文件夹"}</span>
        </div>
        <div className="qzip-selection-summary__actions">
          <Button variant="secondary" icon={<DocumentAddRegular fontSize={20} />} onClick={() => void addFiles()}>添加文件</Button>
          <Button variant="secondary" icon={<FolderAddRegular fontSize={20} />} onClick={() => void addFolder()}>添加文件夹</Button>
        </div>
      </section>

      {inputs.length > 1 ? (
        <div className="qzip-selected-paths">
          {inputs.map((path) => (
            <span key={path}>
              <DocumentRegular fontSize={17} />
              {fileName(path)}
              <button type="button" aria-label={`移除 ${fileName(path)}`} onClick={() => replaceInputs(inputs.filter((item) => item !== path))}>
                <DismissRegular fontSize={15} />
              </button>
            </span>
          ))}
        </div>
      ) : null}

      <section className="qzip-form-sheet">
        <FormRow label="文件名">
          <Input aria-label="文件名" value={name} onChange={(event) => setName(event.target.value)} />
        </FormRow>
        <FormRow label="保存位置">
          <Input
            aria-label="保存位置"
            value={directory}
            onChange={(event) => setDirectory(event.target.value)}
            trailing={
              <button type="button" className="qzip-input-action" aria-label="选择保存位置" onClick={() => void pickOutputFolder()}>
                <FolderOpenRegular fontSize={20} />
              </button>
            }
          />
        </FormRow>
        <FormRow label="格式">
          <SegmentedControl
            options={primaryFormatOptions}
            value={primaryFormatOptions.some((option) => option.value === format) ? format as "sevenZip" | "zip" | "tar" : "sevenZip"}
            onValueChange={(value) => selectFormat(value as ArchiveFormat)}
            ariaLabel="压缩格式"
          />
        </FormRow>
        <FormRow label="压缩方式">
          <SegmentedControl
            options={profiles}
            value={profile === "store" || profile === "maximum" ? "balanced" : profile}
            onValueChange={(value) => setProfile(value as CompressionProfile)}
            ariaLabel="压缩等级"
          />
        </FormRow>
        <FormRow label="密码">
          {passwordOpen ? (
            <Input
              aria-label="压缩密码"
              type="password"
              autoFocus
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              trailing={<EyeRegular fontSize={19} />}
            />
          ) : (
            <button type="button" className="qzip-inline-link" onClick={() => setPasswordOpen(true)}>
              <LockClosedRegular fontSize={18} /> 添加密码
            </button>
          )}
        </FormRow>
      </section>

      <Button
        className="qzip-primary-action qzip-primary-action--wide"
        loading={busy}
        disabled={!inputs.length || !directory || !name}
        icon={<ArchiveRegular fontSize={23} />}
        onClick={() => void start()}
      >
        开始压缩
      </Button>

      <button type="button" className="qzip-more-toggle" aria-expanded={moreOpen} onClick={() => setMoreOpen((value) => !value)}>
        更多设置 <ChevronDownRegular fontSize={18} />
      </button>
      {moreOpen ? (
        <section className="qzip-more-settings">
          <FormRow label="其他格式">
            <SegmentedControl
              options={advancedFormatOptions}
              value={advancedFormatOptions.some((option) => option.value === format) ? format as "tarGz" | "tarXz" : "tarGz"}
              onValueChange={(value) => selectFormat(value as ArchiveFormat)}
              ariaLabel="其他压缩格式"
            />
          </FormRow>
          <label><input type="checkbox" checked={testing} onChange={(event) => setTesting(event.target.checked)} /> 创建完成后测试压缩包完整性</label>
          <label data-disabled={!password || format !== "sevenZip"}><input type="checkbox" disabled={!password || format !== "sevenZip"} checked={encryptHeaders} onChange={(event) => setEncryptHeaders(event.target.checked)} /> 加密文件名（仅 7Z）</label>
        </section>
      ) : null}
      {error ? <p className="qzip-form-error">{error}</p> : null}
      <p className="qzip-page-note"><ShieldCheckmarkRegular fontSize={20} /> 均衡模式在压缩速度与大小之间取得较好平衡，适合大多数场景</p>
    </DetailWorkspace>
  );
}

export function ExtractPage({
  archive,
  session,
  selectedEntries,
  onBack,
  onBrowse,
  onCreated,
  defaultConflictPolicy = "rename",
  initialPassword = ""
}: {
  archive: string;
  session: ArchiveSession;
  selectedEntries?: string[];
  onBack: () => void;
  onBrowse: () => void;
  onCreated: (task: TaskSnapshot) => void;
  defaultConflictPolicy?: ConflictPolicy;
  initialPassword?: string;
}) {
  const [output, setOutput] = useState(archiveClient.isTauri ? "" : "D:\\项目资料");
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>(defaultConflictPolicy);
  const [password, setPassword] = useState(initialPassword);
  const [accepted, setAccepted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hasBlocking = session.risks.some((risk) => !risk.overridable);

  useEffect(() => {
    if (!archiveClient.isTauri) return;
    void archiveClient.suggestExtractOutput(archive, true).then(setOutput).catch(() => undefined);
  }, [archive]);

  const pickOutputFolder = async () => {
    if (!archiveClient.isTauri) {
      setOutput("D:\\项目资料");
      return;
    }
    const picked = await archiveClient.pickInputFolder();
    if (picked) setOutput(picked);
  };
  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      const request = {
        archive,
        output,
        selectedEntries: selectedEntries?.length ? selectedEntries : undefined,
        conflictPolicy,
        password: password || undefined,
        acceptRisk: accepted
      };
      const task = archiveClient.isTauri
        ? await archiveClient.extract(request)
        : makeDemoTask("extract", fileName(archive), output);
      onCreated(task);
      onBack();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <DetailWorkspace title="快速解压" onBack={onBack} className="qzip-extract-page">
      <section className="qzip-archive-identity">
        <div className="qzip-archive-identity__art">
          <ArchiveRegular fontSize={60} />
          <span>{formatLabels[session.format]}</span>
        </div>
        <div>
          <h2>{fileName(archive)}</h2>
          <p>{selectedEntries?.length ? `已选择 ${selectedEntries.length} 项` : "选择的压缩包"}</p>
        </div>
      </section>

      <section className="qzip-stat-grid">
        <Stat label="格式" value={formatLabels[session.format]} />
        <Stat label="压缩大小" value={formatBytes(session.compressedSize)} />
        <Stat label="预计解压大小" value={formatBytes(session.estimatedUncompressedSize)} />
        <Stat label="文件数量" value={`${session.entryCount} 个`} />
      </section>

      <section className="qzip-form-sheet">
        <FormRow label="解压位置">
          <Input
            aria-label="解压位置"
            value={output}
            onChange={(event) => setOutput(event.target.value)}
            trailing={
              <button type="button" className="qzip-input-action" aria-label="选择解压位置" onClick={() => void pickOutputFolder()}>
                <FolderOpenRegular fontSize={20} />
              </button>
            }
          />
        </FormRow>
        <FormRow label="冲突处理">
          <SegmentedControl options={conflictOptions} value={conflictPolicy} onValueChange={(value) => setConflictPolicy(value as ConflictPolicy)} ariaLabel="文件冲突处理" />
        </FormRow>
        <FormRow label="密码（可选）">
          <Input aria-label="解压密码" type="password" value={password} onChange={(event) => setPassword(event.target.value)} trailing={<EyeRegular fontSize={19} />} />
        </FormRow>
      </section>
      <RiskNotice risks={session.risks} accepted={accepted} onAccepted={setAccepted} />
      {error ? <p className="qzip-form-error">{error}</p> : null}
      <div className="qzip-extract-actions">
        <Button
          loading={busy}
          disabled={!output || hasBlocking || (session.risks.length > 0 && !accepted)}
          icon={<ArrowDownloadRegular fontSize={24} />}
          onClick={() => void start()}
        >
          开始解压
        </Button>
        <Button variant="secondary" icon={<FolderOpenRegular fontSize={24} />} onClick={onBrowse}>查看压缩包内容</Button>
      </div>
      <p className="qzip-page-note"><ShieldCheckmarkRegular fontSize={20} /> 轻压保障解压安全，不上传您的文件到任何服务器</p>
    </DetailWorkspace>
  );
}

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
  const [search, setSearch] = useState("");
  const [directory, setDirectory] = useState("");
  const [entries, setEntries] = useState<ArchiveEntry[]>(archiveClient.isTauri ? [] : demoEntries);
  const [loading, setLoading] = useState(archiveClient.isTauri);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [propertiesOpen, setPropertiesOpen] = useState(false);

  useEffect(() => {
    if (!archiveClient.isTauri) return;
    void archiveClient.entries(session.sessionId, directory || undefined)
      .then((page) => setEntries(page.entries))
      .finally(() => setLoading(false));
  }, [directory, session.sessionId]);

  const visibleEntries = !archiveClient.isTauri && directory ? [] : entries;
  const shown = visibleEntries.filter((entry) => entry.displayName.toLowerCase().includes(search.toLowerCase()));
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
  function openDirectory(entry: ArchiveEntry) {
    if (!entry.isDirectory) return;
    setDirectory(entry.path.endsWith("/") ? entry.path : `${entry.path}/`);
    setSelected(new Set());
    setSearch("");
  }
  function navigateBreadcrumb(index: number) {
    setDirectory(index < 0 ? "" : `${breadcrumbs.slice(0, index + 1).join("/")}/`);
    setSelected(new Set());
  }
  async function addToArchive(folder: boolean) {
    const inputs = archiveClient.isTauri
      ? folder
        ? [await archiveClient.pickInputFolder()].filter((value): value is string => Boolean(value))
        : await archiveClient.pickInputPaths(false)
      : [folder ? "D:\\新增文件夹" : "D:\\新增文件.txt"];
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
        <button type="button" className="qzip-square-action" aria-label="返回快速解压" onClick={onBack}>
          <ArrowLeftRegular fontSize={24} />
        </button>
        <div className="qzip-browser-page__title-icon"><ArchiveRegular fontSize={28} /><span>{formatLabels[session.format]}</span></div>
        <h1>{fileName(archive)}</h1>
        <div className="qzip-browser-actions">
          <button type="button" onClick={() => void addToArchive(false)}><AddCircleRegular fontSize={22} /> 添加 <ChevronDownRegular fontSize={16} /></button>
          <button type="button" onClick={() => onExtract([...selected])}><ArrowDownloadRegular fontSize={22} /> 解压 <ChevronDownRegular fontSize={16} /></button>
          <button type="button" onClick={() => void testArchive()}><ShieldCheckmarkRegular fontSize={22} /> 测试 <ChevronDownRegular fontSize={16} /></button>
          <button type="button" aria-expanded={propertiesOpen} onClick={() => setPropertiesOpen((value) => !value)}><MoreHorizontalRegular fontSize={22} /> 更多 <ChevronDownRegular fontSize={16} /></button>
        </div>
        {propertiesOpen ? (
          <div className="qzip-browser-properties">
            <strong>压缩包属性</strong>
            <span>{formatLabels[session.format]} · {session.entryCount} 项</span>
            <button type="button" onClick={() => void addToArchive(true)}><FolderAddRegular fontSize={18} /> 添加文件夹</button>
            <button type="button" onClick={onClose}><DismissRegular fontSize={18} /> 关闭压缩包</button>
          </div>
        ) : null}
      </header>

      <Card className="qzip-browser-card">
        <div className="qzip-browser-toolbar">
          <nav className="qzip-breadcrumb" aria-label="压缩包路径">
            <button type="button" aria-label="根目录" onClick={() => navigateBreadcrumb(-1)}><HomeRegular fontSize={21} /></button>
            {breadcrumbs.map((part, index) => (
              <span key={`${part}-${index}`}>
                <ChevronRightRegular fontSize={17} />
                <button type="button" onClick={() => navigateBreadcrumb(index)}>{part}</button>
              </span>
            ))}
          </nav>
          <Input aria-label="搜索压缩包内容" placeholder="搜索" value={search} onChange={(event) => setSearch(event.target.value)} trailing={<SearchRegular fontSize={20} />} />
        </div>
        <div className="qzip-entry-table" role="table">
          <div className="qzip-entry-table__head" role="row">
            <span>名称</span><span>大小</span><span>类型</span><span>修改时间</span>
          </div>
          {loading ? <Empty icon={<ArrowClockwiseRegular fontSize={34} className="qzip-spin" />} text="正在读取压缩包目录…" /> : shown.map((entry) => (
            <button
              className="qzip-entry-row"
              key={entry.path}
              role="row"
              onClick={() => toggleSelected(entry.path)}
              onDoubleClick={() => openDirectory(entry)}
              data-selected={selected.has(entry.path)}
            >
              <span>{entry.isDirectory ? <FolderRegular fontSize={23} /> : <DocumentRegular fontSize={23} />}{entry.displayName}{entry.encrypted ? <LockClosedRegular fontSize={14} /> : null}</span>
              <span>{entry.isDirectory ? "—" : formatBytes(entry.size)}</span>
              <span>{formatType(entry)}</span>
              <span>{entry.modifiedAt?.replace("T", " ").slice(0, 16) ?? "—"}</span>
            </button>
          ))}
          {!shown.length && !loading ? <Empty icon={<SearchRegular fontSize={34} />} text={directory ? "此文件夹为空" : "没有匹配的文件"} /> : null}
        </div>
        <footer className="qzip-browser-footer">
          <span>共 {session.entryCount} 项</span>
          <span>原始大小：{formatBytes(session.estimatedUncompressedSize)}</span>
          <span>压缩后大小：{formatBytes(session.compressedSize)}</span>
          <strong>压缩率 {ratio.toFixed(1)}%</strong>
        </footer>
      </Card>
    </section>
  );
}

export function TaskCenter({
  tasks,
  onBack,
  onClear,
  onCancel,
  onRetry
}: {
  tasks: TaskSnapshot[];
  onBack: () => void;
  onClear: () => void;
  onCancel: (id: string) => void;
  onRetry: (id: string, password?: string) => void;
}) {
  const [tab, setTab] = useState<"active" | "completed" | "failed">("active");
  const active = tasks.filter((task) => ["queued", "scanning", "running", "cancelling"].includes(task.status));
  const completed = tasks.filter((task) => task.status === "completed");
  const failed = tasks.filter((task) => ["failed", "cancelled"].includes(task.status));
  const taskGroups = [
    { id: "active", title: "进行中", tasks: active },
    { id: "completed", title: "已完成", tasks: completed },
    { id: "failed", title: "失败", tasks: failed }
  ] as const;

  function selectGroup(group: typeof tab) {
    setTab(group);
    document.getElementById(`qzip-task-group-${group}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  return (
    <section className="qzip-task-center">
      <aside className="qzip-task-sidebar">
        <h1>任务中心</h1>
        <button type="button" data-active={tab === "active"} onClick={() => selectGroup("active")}><ArrowClockwiseRegular fontSize={23} />进行中 <em>{active.length}</em></button>
        <button type="button" data-active={tab === "completed"} onClick={() => selectGroup("completed")}><CheckmarkCircleRegular fontSize={23} />已完成 <em>{completed.length}</em></button>
        <button type="button" data-active={tab === "failed"} onClick={() => selectGroup("failed")}><ErrorCircleRegular fontSize={23} />失败 <em>{failed.length}</em></button>
        <div className="qzip-task-sidebar__footer">
          <button type="button" onClick={onBack}><HomeRegular fontSize={20} /> 返回首页</button>
          <button type="button" onClick={onClear}><DeleteRegular fontSize={20} /> 清空已完成</button>
        </div>
      </aside>
      <section className="qzip-task-content">
        <header>
          <h2>进行中（{active.length}）</h2>
          {active.length ? <div><button type="button" disabled title="RC1 暂不支持暂停任务">全部暂停</button><button type="button" onClick={() => active.forEach((task) => onCancel(task.taskId))}>全部取消</button></div> : null}
        </header>
        <div className="qzip-task-list">
          {taskGroups.map((group, index) => (
            <section
              id={`qzip-task-group-${group.id}`}
              className="qzip-task-group"
              data-primary={index === 0}
              key={group.id}
            >
              {index > 0 ? <h2>{group.title}（{group.tasks.length}）</h2> : null}
              {group.tasks.length
                ? group.tasks.map((task) => <TaskCard key={task.taskId} task={task} onCancel={onCancel} onRetry={onRetry} />)
                : index === 0 ? <Card className="qzip-task-empty"><Empty icon={<ArchiveRegular fontSize={40} />} text="暂无进行中任务" /></Card> : null}
            </section>
          ))}
        </div>
      </section>
    </section>
  );
}

function TaskCard({
  task,
  onCancel,
  onRetry
}: {
  task: TaskSnapshot;
  onCancel: (id: string) => void;
  onRetry: (id: string, password?: string) => void;
}) {
  const active = ["queued", "scanning", "running", "cancelling"].includes(task.status);
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const needsPassword = task.error?.code === "WRONG_PASSWORD";
  const percent = task.progress?.percent ?? (task.status === "completed" ? 100 : 0);
  const status = task.status === "completed"
    ? "已完成"
    : task.status === "failed"
      ? `失败：${task.error?.message ?? "处理失败"}`
      : task.status === "cancelled"
        ? "已取消"
        : task.operation === "extract"
          ? "正在解压"
          : task.status === "queued"
            ? "等待中"
            : "正在压缩";

  return (
    <Card className="qzip-task-card" data-status={task.status}>
      <div className="qzip-task-card__icon"><ArchiveRegular fontSize={30} /><span>{task.operation === "extract" ? "ZIP" : "7Z"}</span></div>
      <div className="qzip-task-card__body">
        <div className="qzip-task-card__heading">
          <strong>{task.displayName}</strong>
          <span className={`qzip-status qzip-status--${task.status}`}>{status}</span>
          {active ? <b>{percent}%</b> : null}
        </div>
        {active ? <Progress value={Math.max(percent, task.status === "queued" ? 4 : 0)} /> : null}
        <p className="qzip-task-card__meta">
          {active ? <>当前文件：{task.progress?.currentEntry ?? "准备处理"} <i /> 已用时间：{formatElapsed(task.progress?.elapsedSeconds)}</> : null}
          {task.status === "completed" ? <>完成时间：{formatTaskTimestamp(task.updatedAt)}</> : null}
          {task.status === "failed" || task.status === "cancelled" ? <>失败时间：{formatTaskTimestamp(task.updatedAt)}</> : null}
        </p>
        {needsPassword && showPassword ? <Input aria-label="重试密码" type="password" value={password} onChange={(event) => setPassword(event.target.value)} placeholder="请输入正确密码" /> : null}
      </div>
      <div className="qzip-task-card__actions">
        {active ? <Button variant="icon" aria-label="暂停任务（暂不支持）" disabled title="RC1 暂不支持暂停任务" icon={<PauseRegular fontSize={22} />} /> : null}
        {active ? <Button variant="icon" aria-label="取消任务" icon={<DismissRegular fontSize={22} />} onClick={() => onCancel(task.taskId)} /> : null}
        {task.status === "failed" && task.retryable ? (
          <Button
            variant="danger"
            icon={<WarningRegular fontSize={19} />}
            disabled={showPassword && needsPassword && !password}
            onClick={() => showPassword ? onRetry(task.taskId, password || undefined) : setShowPassword(true)}
          >
            {showPassword ? "确认重试" : needsPassword ? "重新输入密码" : "重试"}
          </Button>
        ) : null}
        {task.status === "completed" && task.output ? <Button variant="secondary" icon={<OpenRegular fontSize={19} />} onClick={() => void archiveClient.open(task.output!)}>打开结果</Button> : null}
        {task.output && task.status !== "failed" ? <Button variant="secondary" onClick={() => void archiveClient.reveal(task.output!)}>打开位置</Button> : null}
      </div>
    </Card>
  );
}

function DetailWorkspace({
  title,
  onBack,
  className,
  children
}: {
  title: string;
  onBack: () => void;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <section className={`qzip-detail-page ${className ?? ""}`}>
      <Card className="qzip-detail-panel">
        <header className="qzip-detail-panel__header">
          <button type="button" aria-label="返回首页" onClick={onBack}><ArrowLeftRegular fontSize={26} /></button>
          <h1>{title}</h1>
        </header>
        {children}
      </Card>
    </section>
  );
}

function FormRow({ label, children }: { label: string; children: React.ReactNode }) {
  return <div className="qzip-form-row"><strong>{label}</strong><div>{children}</div></div>;
}

function Stat({ label, value }: { label: string; value: string }) {
  return <div className="qzip-stat"><span>{label}</span><strong>{value}</strong></div>;
}

function Empty({ icon, text }: { icon: React.ReactNode; text: string }) {
  return <div className="qzip-work-empty">{icon}<p>{text}</p></div>;
}

export type { Page };
