import { useCallback, useEffect, useRef, useState } from "react";
import {
  AddCircleRegular,
  ArchiveRegular,
  ArrowClockwiseRegular,
  ArrowDownloadRegular,
  ArrowLeftRegular,
  ChevronDownRegular,
  ChevronRightRegular,
  DeleteRegular,
  DismissRegular,
  DocumentAddRegular,
  DocumentRegular,
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
  BackendCapabilities,
  CompressionProfile,
  ConflictPolicy,
  CreateTaskRequest,
  TaskSnapshot
} from "../../contracts/archive";
import { archiveClient } from "../../lib/archiveClient";
import { localize, useI18n, type AppLocale } from "../../lib/i18n";
import { joinOutputPath, splitOutputPath, suggestCreateOutputLocally } from "./archivePath";

type Page = "home" | "create" | "extract" | "batchExtract" | "browser" | "tasks";

const primaryFormatOptions = [
  { value: "sevenZip", label: "7Z" },
  { value: "zip", label: "ZIP" },
  { value: "tar", label: "TAR" }
] as const;
const advancedFormatOptions = [
  { value: "tarGz", label: "TAR.GZ" },
  { value: "tarXz", label: "TAR.XZ" }
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
  unknown: "?"
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

function errorMessage(reason: unknown) {
  if (reason && typeof reason === "object" && "message" in reason && typeof reason.message === "string") return reason.message;
  return String(reason);
}

function formatType(entry: ArchiveEntry, locale: AppLocale) {
  if (entry.isDirectory) return localize(locale, "文件夹", "Folder");
  const extension = entry.displayName.split(".").pop()?.toUpperCase();
  return extension ? localize(locale, `${extension} 文件`, `${extension} file`) : localize(locale, "文件", "File");
}

function formatElapsed(seconds = 0) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = Math.floor(seconds % 60);
  return [hours, minutes, rest].map((value) => String(value).padStart(2, "0")).join(":");
}

function formatTaskTimestamp(value: number, locale: AppLocale) {
  // Rust task-runtime timestamps are Unix seconds; demo/test tasks use JS
  // milliseconds. Accept both while persisted task history is migrated.
  const milliseconds = value < 100_000_000_000 ? value * 1000 : value;
  const date = new Date(milliseconds);
  return Number.isNaN(date.getTime()) ? "—" : date.toLocaleString(locale);
}

const taskFormatSuffixes = [
  [".tar.gz", "TAR.GZ", "tgz"],
  [".tar.xz", "TAR.XZ", "txz"],
  [".tgz", "TAR.GZ", "tgz"],
  [".txz", "TAR.XZ", "txz"],
  [".7z", "7Z", "7z"],
  [".zip", "ZIP", "zip"],
  [".rar", "RAR", "rar"],
  [".tar", "TAR", "tar"],
  [".gz", "GZ", "gz"],
  [".xz", "XZ", "xz"],
  [".bz2", "BZ2", "bz2"],
  [".iso", "ISO", "iso"],
  [".cab", "CAB", "cab"],
  [".wim", "WIM", "wim"]
] as const;

function taskFormatInfo(task: TaskSnapshot) {
  const candidates = task.operation === "create"
    ? [task.output, task.displayName]
    : [task.displayName, task.output];
  for (const candidate of candidates) {
    const normalized = candidate?.toLowerCase();
    if (!normalized) continue;
    const match = taskFormatSuffixes.find(([suffix]) => normalized.endsWith(suffix));
    if (match) return match;
  }
  return undefined;
}

function taskFormatLabel(task: TaskSnapshot) {
  return taskFormatInfo(task)?.[1] ?? "—";
}

function taskFormatIcon(task: TaskSnapshot) {
  return `/file-types/${taskFormatInfo(task)?.[2] ?? "archive"}.ico`;
}

function isActiveTask(task: TaskSnapshot) {
  return ["queued", "scanning", "running", "cancelling"].includes(task.status);
}

function taskOperationLabel(operation: TaskSnapshot["operation"], locale: AppLocale) {
  const labels: Record<TaskSnapshot["operation"], [string, string]> = {
    create: ["创建压缩包", "Create archive"],
    extract: ["解压", "Extract"],
    list: ["读取内容", "List contents"],
    test: ["完整性测试", "Test integrity"],
    update: ["更新压缩包", "Update archive"]
  };
  const [chinese, english] = labels[operation];
  return localize(locale, chinese, english);
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
  const { text } = useI18n();
  if (!risks.length) return null;
  const mayContinue = risks.some((risk) => risk.overridable);
  return (
    <aside className="qzip-risk-notice">
      <WarningRegular fontSize={22} />
      <div>
        <strong>{text("安全检查提示", "Security notice")}</strong>
        <p>{risks.map((risk) => risk.message).join("；")}</p>
        {mayContinue ? (
          <label>
            <input type="checkbox" checked={accepted} onChange={(event) => onAccepted(event.target.checked)} />
            {text("我已了解本次风险并继续", "I understand the risk and want to continue")}
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
  const { locale, text } = useI18n();
  const [inputs, setInputs] = useState<string[]>(() => [...new Set(initialInputs)]);
  const [format, setFormat] = useState<ArchiveFormat>(defaultFormat);
  const [profile, setProfile] = useState<CompressionProfile>(defaultProfile);
  const initialOutput = splitOutputPath(
    suggestCreateOutputLocally(initialInputs, defaultFormat, locale)
      ?? (archiveClient.isTauri ? "" : text("D:\\示例\\项目资料.7z", "D:\\Examples\\Project.7z"))
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
  const [capabilities, setCapabilities] = useState<BackendCapabilities | null>(null);
  const submittingRef = useRef(false);
  const suggestionGeneration = useRef(0);
  const initialSuggestion = useRef({ inputs: [...new Set(initialInputs)], format: defaultFormat });
  const writableFormats = capabilities?.writableFormats;
  const primaryOptions = primaryFormatOptions.map((option) => ({
    ...option,
    disabled: Boolean(writableFormats && !writableFormats.includes(option.value))
  }));
  const advancedOptions = advancedFormatOptions.map((option) => ({
    ...option,
    disabled: Boolean(writableFormats && !writableFormats.includes(option.value))
  }));
  const profileOptions = [
    { value: "fast", label: text("快速", "Fast") },
    { value: "balanced", label: text("均衡", "Balanced") },
    { value: "small", label: text("更小", "Smaller") }
  ];

  const applySuggestedOutput = useCallback((path: string) => {
    const next = splitOutputPath(path);
    setDirectory(next.directory);
    setName(next.name);
  }, []);
  const suggestOutput = useCallback((nextInputs: string[], nextFormat: ArchiveFormat) => {
    const local = suggestCreateOutputLocally(nextInputs, nextFormat, locale);
    if (local) applySuggestedOutput(local);
    if (!nextInputs.length || !archiveClient.isTauri) return;
    const generation = ++suggestionGeneration.current;
    void archiveClient.suggestCreateOutput(nextInputs, nextFormat)
      .then((path) => {
        if (generation === suggestionGeneration.current) applySuggestedOutput(path);
      })
      .catch(() => undefined);
  }, [applySuggestedOutput, locale]);
  const replaceInputs = (nextInputs: string[]) => {
    const unique = [...new Set(nextInputs)];
    setInputs(unique);
    suggestOutput(unique, format);
  };
  const selectFormat = (nextFormat: ArchiveFormat) => {
    if (writableFormats && !writableFormats.includes(nextFormat)) return;
    setFormat(nextFormat);
    suggestOutput(inputs, nextFormat);
  };
  const addFiles = async () => {
    if (!archiveClient.isTauri) {
      replaceInputs([text("D:\\项目资料", "D:\\Project")]);
      return;
    }
    const picked = await archiveClient.pickInputPaths(false);
    replaceInputs([...inputs, ...picked]);
  };
  const addFolder = async () => {
    if (!archiveClient.isTauri) {
      setInputs([text("D:\\项目资料", "D:\\Project")]);
      return;
    }
    const picked = await archiveClient.pickInputFolder();
    if (picked) replaceInputs([...inputs, picked]);
  };
  const pickOutputFolder = async () => {
    if (!archiveClient.isTauri) {
      setDirectory(text("D:\\桌面", "D:\\Desktop"));
      return;
    }
    const picked = await archiveClient.pickInputFolder();
    if (picked) setDirectory(picked);
  };

  useEffect(() => {
    if (!archiveClient.isTauri) return;
    void archiveClient.capabilities().then(setCapabilities).catch(() => undefined);
  }, []);

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
      setError(text("请先选择要压缩的文件，并填写保存位置和文件名。", "Select files to compress, then enter a destination and file name."));
      return;
    }
    if (format === "tar" && password) {
      setError(text("TAR 格式不支持密码，请改用 7Z 或 ZIP。", "TAR does not support passwords. Use 7Z or ZIP instead."));
      return;
    }
    if (writableFormats && !writableFormats.includes(format)) {
      setError(text("当前压缩后端不支持所选格式，请选择可用格式。", "The current compression backend does not support this format."));
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
      onCreated(archiveClient.isTauri ? await archiveClient.create(request) : makeDemoTask("create", name || text("项目资料.7z", "project.7z"), output));
      onOpenTasks();
    } catch (reason) {
      setError(String(reason));
    } finally {
      submittingRef.current = false;
      setBusy(false);
    }
  };

  const summaryName = inputs.length === 1 ? fileName(inputs[0] ?? "") : text(`${inputs.length} 个对象`, `${inputs.length} items`);
  return (
    <DetailWorkspace title={text("创建压缩包", "Create archive")} onBack={onBack} className="qzip-create-page">
      <section className="qzip-selection-summary">
        <div className="qzip-selection-summary__icon"><FolderRegular fontSize={38} /></div>
        <div>
          <strong>{inputs.length ? summaryName : text("选择要压缩的文件", "Choose files to compress")}</strong>
          <span>{inputs.length ? text(`${inputs.length} 个已选对象`, `${inputs.length} selected`) : text("可添加文件或整个文件夹", "Add files or an entire folder")}</span>
        </div>
        <div className="qzip-selection-summary__actions">
          <Button variant="secondary" icon={<DocumentAddRegular fontSize={20} />} onClick={() => void addFiles()}>{text("添加文件", "Add files")}</Button>
          <Button variant="secondary" icon={<FolderAddRegular fontSize={20} />} onClick={() => void addFolder()}>{text("添加文件夹", "Add folder")}</Button>
        </div>
      </section>

      {inputs.length > 1 ? (
        <div className="qzip-selected-paths">
          {inputs.map((path) => (
            <span key={path}>
              <DocumentRegular fontSize={17} />
              {fileName(path)}
              <button type="button" aria-label={text(`移除 ${fileName(path)}`, `Remove ${fileName(path)}`)} onClick={() => replaceInputs(inputs.filter((item) => item !== path))}>
                <DismissRegular fontSize={15} />
              </button>
            </span>
          ))}
        </div>
      ) : null}

      <section className="qzip-form-sheet">
        <FormRow label={text("文件名", "File name")}>
          <Input aria-label={text("文件名", "File name")} value={name} onChange={(event) => setName(event.target.value)} />
        </FormRow>
        <FormRow label={text("保存位置", "Destination")}>
          <Input
            aria-label={text("保存位置", "Destination")}
            value={directory}
            onChange={(event) => setDirectory(event.target.value)}
            trailing={
              <button type="button" className="qzip-input-action" aria-label={text("选择保存位置", "Choose destination")} onClick={() => void pickOutputFolder()}>
                <FolderOpenRegular fontSize={20} />
              </button>
            }
          />
        </FormRow>
        <FormRow label={text("格式", "Format")}>
          <SegmentedControl
            options={primaryOptions}
            value={primaryFormatOptions.some((option) => option.value === format) ? format as "sevenZip" | "zip" | "tar" : "sevenZip"}
            onValueChange={(value) => selectFormat(value as ArchiveFormat)}
            ariaLabel={text("压缩格式", "Archive format")}
          />
        </FormRow>
        <FormRow label={text("压缩方式", "Compression level")}>
          <SegmentedControl
            options={profileOptions}
            value={profile === "store" || profile === "maximum" ? "balanced" : profile}
            onValueChange={(value) => setProfile(value as CompressionProfile)}
            ariaLabel={text("压缩等级", "Compression level")}
          />
        </FormRow>
        <FormRow label={text("密码", "Password")}>
          {passwordOpen ? (
            <Input
              aria-label={text("压缩密码", "Archive password")}
              type="password"
              autoFocus
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              trailing={<EyeRegular fontSize={19} />}
            />
          ) : (
            <button type="button" className="qzip-inline-link" disabled={capabilities?.supportsPassword === false} onClick={() => setPasswordOpen(true)}>
              <LockClosedRegular fontSize={18} /> {text("添加密码", "Add password")}
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
        {text("开始压缩", "Start compression")}
      </Button>

      <button type="button" className="qzip-more-toggle" aria-expanded={moreOpen} onClick={() => setMoreOpen((value) => !value)}>
        {text("更多设置", "More settings")} <ChevronDownRegular fontSize={18} />
      </button>
      {moreOpen ? (
        <section className="qzip-more-settings">
          <FormRow label={text("其他格式", "Other formats")}>
            <SegmentedControl
              options={advancedOptions}
              value={advancedFormatOptions.some((option) => option.value === format) ? format as "tarGz" | "tarXz" : "tarGz"}
              onValueChange={(value) => selectFormat(value as ArchiveFormat)}
              ariaLabel={text("其他压缩格式", "Other archive formats")}
            />
          </FormRow>
          <label><input type="checkbox" checked={testing} onChange={(event) => setTesting(event.target.checked)} /> {text("创建完成后测试压缩包完整性", "Test archive integrity after creation")}</label>
          <label data-disabled={!password || format !== "sevenZip"}><input type="checkbox" disabled={!password || format !== "sevenZip"} checked={encryptHeaders} onChange={(event) => setEncryptHeaders(event.target.checked)} /> {text("加密文件名（仅 7Z）", "Encrypt file names (7Z only)")}</label>
        </section>
      ) : null}
      {error ? <p className="qzip-form-error">{error}</p> : null}
      <p className="qzip-page-note"><ShieldCheckmarkRegular fontSize={20} /> {text("均衡模式在压缩速度与大小之间取得较好平衡，适合大多数场景", "Balanced mode offers a good tradeoff between speed and size for most uses")}</p>
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
  const { text } = useI18n();
  const [output, setOutput] = useState(archiveClient.isTauri ? "" : text("D:\\项目资料", "D:\\Project"));
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>(defaultConflictPolicy);
  const [password, setPassword] = useState(initialPassword);
  const [accepted, setAccepted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hasBlocking = session.risks.some((risk) => !risk.overridable);
  const conflictOptions = [
    { value: "rename", label: text("重命名", "Rename") },
    { value: "overwrite", label: text("覆盖", "Overwrite") },
    { value: "skip", label: text("跳过", "Skip") }
  ];

  useEffect(() => {
    if (!archiveClient.isTauri) return;
    void archiveClient.suggestExtractOutput(archive, true).then(setOutput).catch(() => undefined);
  }, [archive]);

  const pickOutputFolder = async () => {
    if (!archiveClient.isTauri) {
      setOutput(text("D:\\项目资料", "D:\\Project"));
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
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <DetailWorkspace title={text("快速解压", "Quick extract")} onBack={onBack} className="qzip-extract-page">
      <section className="qzip-archive-identity">
        <div className="qzip-archive-identity__art">
          <ArchiveRegular fontSize={60} />
          <span>{formatLabels[session.format]}</span>
        </div>
        <div>
          <h2>{fileName(archive)}</h2>
          <p>{selectedEntries?.length ? text(`已选择 ${selectedEntries.length} 项`, `${selectedEntries.length} items selected`) : text("选择的压缩包", "Selected archive")}</p>
        </div>
      </section>

      <section className="qzip-stat-grid">
        <Stat label={text("格式", "Format")} value={formatLabels[session.format]} />
        <Stat label={text("压缩大小", "Compressed size")} value={formatBytes(session.compressedSize)} />
        <Stat label={text("预计解压大小", "Estimated extracted size")} value={formatBytes(session.estimatedUncompressedSize)} />
        <Stat label={text("文件数量", "Items")} value={text(`${session.entryCount} 个`, `${session.entryCount}`)} />
      </section>

      <section className="qzip-form-sheet">
        <FormRow label={text("解压位置", "Extract to")}>
          <Input
            aria-label={text("解压位置", "Extract to")}
            value={output}
            onChange={(event) => setOutput(event.target.value)}
            trailing={
              <button type="button" className="qzip-input-action" aria-label={text("选择解压位置", "Choose extraction folder")} onClick={() => void pickOutputFolder()}>
                <FolderOpenRegular fontSize={20} />
              </button>
            }
          />
        </FormRow>
        <FormRow label={text("冲突处理", "File conflicts")}>
          <SegmentedControl options={conflictOptions} value={conflictPolicy} onValueChange={(value) => setConflictPolicy(value as ConflictPolicy)} ariaLabel={text("文件冲突处理", "File conflicts")} />
        </FormRow>
        <FormRow label={text("密码（可选）", "Password (optional)")}>
          <Input aria-label={text("解压密码", "Extraction password")} type="password" value={password} onChange={(event) => setPassword(event.target.value)} trailing={<EyeRegular fontSize={19} />} />
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
          {text("开始解压", "Start extraction")}
        </Button>
        <Button variant="secondary" icon={<FolderOpenRegular fontSize={24} />} onClick={onBrowse}>{text("查看压缩包内容", "Browse archive")}</Button>
      </div>
      <p className="qzip-page-note"><ShieldCheckmarkRegular fontSize={20} /> {text("轻压保障解压安全，不上传您的文件到任何服务器", "QZip extracts locally and never uploads your files")}</p>
    </DetailWorkspace>
  );
}

export function BatchExtractPage({
  archives,
  onBack,
  onStarted,
  defaultConflictPolicy = "rename"
}: {
  archives: string[];
  onBack: () => void;
  onStarted: (tasks: TaskSnapshot[], failures: { archive: string; message: string }[]) => void;
  defaultConflictPolicy?: ConflictPolicy;
}) {
  const { text } = useI18n();
  const [busy, setBusy] = useState(false);
  const [completed, setCompleted] = useState(0);
  const [current, setCurrent] = useState("");

  async function start() {
    if (busy || !archives.length) return;
    setBusy(true);
    setCompleted(0);
    const tasks: TaskSnapshot[] = [];
    const failures: { archive: string; message: string }[] = [];
    for (const target of archives) {
      setCurrent(target);
      try {
        const prepared = await archiveClient.prepare(target);
        if (prepared.risks.length) throw new Error(text("需要单独打开并确认安全风险", "Open separately to review security risks"));
        const output = await archiveClient.suggestExtractOutput(target, true);
        tasks.push(await archiveClient.extract({
          archive: target,
          output,
          conflictPolicy: defaultConflictPolicy,
          acceptRisk: false
        }));
      } catch (reason) {
        failures.push({ archive: target, message: errorMessage(reason) });
      }
      setCompleted((value) => value + 1);
    }
    setBusy(false);
    setCurrent("");
    onStarted(tasks, failures);
  }

  return (
    <DetailWorkspace title={text("批量解压", "Batch extraction")} onBack={onBack} className="qzip-batch-extract-page">
      <div className="qzip-batch-summary">
        <ArchiveRegular fontSize={36} />
        <div><strong>{text(`${archives.length} 个压缩包`, `${archives.length} archives`)}</strong><span>{text("每个压缩包将解压到所在位置的同名文件夹", "Each archive will be extracted to a same-name folder beside it")}</span></div>
      </div>
      <div className="qzip-batch-list">
        {archives.map((target, index) => (
          <div key={target} data-current={busy && target === current}>
            <ArchiveRegular fontSize={21} />
            <span>{fileName(target)}</span>
            <em>{index < completed ? text("已处理", "Processed") : busy && target === current ? text("正在检查", "Checking") : text("等待", "Waiting")}</em>
          </div>
        ))}
      </div>
      <Button className="qzip-primary-action qzip-primary-action--wide" loading={busy} disabled={!archives.length} icon={<ArrowDownloadRegular fontSize={22} />} onClick={() => void start()}>
        {text("开始批量解压", "Start batch extraction")}
      </Button>
      <p className="qzip-page-note"><ShieldCheckmarkRegular fontSize={20} /> {text("加密包或需要风险确认的压缩包会跳过，请随后单独打开处理", "Encrypted archives and archives requiring risk confirmation are skipped for individual review")}</p>
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
  const { locale, text } = useI18n();
  const [search, setSearch] = useState("");
  const [directory, setDirectory] = useState("");
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
      void archiveClient.entries(session.sessionId, directory || undefined, search || undefined)
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
  }, [directory, search, session.sessionId]);
  useEffect(() => {
    if (!archiveClient.isTauri || loading || loadError || reportedSessionRef.current === session.sessionId) return;
    reportedSessionRef.current = session.sessionId;
    void archiveClient.recordPerformanceMarker("archive-list-first-page");
  }, [loadError, loading, session.sessionId]);

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
      const page = await archiveClient.entries(session.sessionId, directory || undefined, search || undefined, nextOffset);
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
            <span>{text("名称", "Name")}</span><span>{text("大小", "Size")}</span><span>{text("类型", "Type")}</span><span>{text("修改时间", "Modified")}</span>
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
              <span>{openingEntry === entry.path ? <ArrowClockwiseRegular fontSize={23} className="qzip-spin" /> : entry.isDirectory ? <FolderRegular fontSize={23} /> : <DocumentRegular fontSize={23} />}{entry.displayName}{entry.encrypted ? <LockClosedRegular fontSize={14} /> : null}</span>
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

export function TaskCenter({
  tasks,
  focusTaskId,
  onBack,
  onClear,
  onCancel,
  onRetry
}: {
  tasks: TaskSnapshot[];
  focusTaskId?: string;
  onBack: () => void;
  onClear: () => void;
  onCancel: (id: string) => void;
  onRetry: (id: string, password?: string) => void;
}) {
  const { text } = useI18n();
  const active = tasks.filter(isActiveTask);
  const orderedTasks = [...tasks].sort((left, right) => {
    const activeDifference = Number(isActiveTask(right)) - Number(isActiveTask(left));
    if (activeDifference) return activeDifference;
    return right.updatedAt - left.updatedAt || right.createdAt - left.createdAt || right.taskId.localeCompare(left.taskId);
  });

  return (
    <section className="qzip-task-center qzip-task-center--single-list">
      <section className="qzip-task-content">
        <header className="qzip-task-content__header">
          <div>
            <h1>{text("任务中心", "Task center")}</h1>
            <span>{text(`共 ${tasks.length} 个任务`, `${tasks.length} tasks total`)}</span>
          </div>
          <div className="qzip-task-content__toolbar">
            <Button variant="tertiary" icon={<HomeRegular fontSize={19} />} onClick={onBack}>{text("返回首页", "Back to home")}</Button>
            {active.length ? <Button variant="danger" icon={<DismissRegular fontSize={19} />} onClick={() => active.forEach((task) => onCancel(task.taskId))}>{text("取消全部进行中", "Cancel active tasks")}</Button> : null}
            <Button variant="secondary" icon={<DeleteRegular fontSize={19} />} onClick={onClear}>{text("清空已结束", "Clear ended tasks")}</Button>
          </div>
        </header>
        <div className="qzip-task-list">
          {orderedTasks.length
            ? orderedTasks.map((task) => <TaskCard key={task.taskId} task={task} focused={task.taskId === focusTaskId} onCancel={onCancel} onRetry={onRetry} />)
            : <Card className="qzip-task-empty"><Empty icon={<ArchiveRegular fontSize={40} />} text={text("暂无任务记录", "No tasks yet")} /></Card>}
        </div>
      </section>
    </section>
  );
}

function TaskCard({
  task,
  focused,
  onCancel,
  onRetry
}: {
  task: TaskSnapshot;
  focused: boolean;
  onCancel: (id: string) => void;
  onRetry: (id: string, password?: string) => void;
}) {
  const { locale, text } = useI18n();
  const active = isActiveTask(task);
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const needsPassword = task.error?.code === "WRONG_PASSWORD";
  const percent = task.progress?.percent ?? (task.status === "completed" ? 100 : 0);
  const phase = task.progress?.phase;
  const phaseLabel = task.status === "queued"
    ? text("等待开始", "Waiting to start")
    : task.status === "scanning" || phase === "scanning"
      ? text("正在检查压缩包", "Checking archive")
      : task.status === "cancelling"
        ? text("正在取消", "Cancelling")
        : phase === "committing"
          ? text("正在整理结果", "Finalizing result")
          : phase === "creating"
            ? text("正在创建压缩包", "Creating archive")
            : phase === "testing"
              ? text("正在测试压缩包", "Testing archive")
              : phase === "updating"
                ? text("正在更新压缩包", "Updating archive")
                : task.operation === "extract"
                  ? text("正在解压", "Extracting")
                  : text("正在压缩", "Compressing");
  const status = task.status === "completed"
    ? text("已完成", "Completed")
    : task.status === "failed"
      ? needsPassword
        ? text("失败：密码错误", "Failed: incorrect password")
        : text(`失败：${task.error?.message ?? "处理失败"}`, `Failed: ${task.error?.message ?? "Processing failed"}`)
      : task.status === "cancelled"
        ? text("已取消", "Cancelled")
        : task.operation === "extract"
          ? text("正在解压", "Extracting")
          : task.status === "queued"
            ? text("等待中", "Waiting")
            : text("正在压缩", "Compressing");
  const outputLabel = task.operation === "extract"
    ? text("解压位置", "Extracted to")
    : task.operation === "create"
      ? text("压缩包位置", "Archive saved to")
      : text("目标位置", "Destination");
  const hasDetails = Boolean(task.error || task.warnings.length);

  return (
    <Card className="qzip-task-card" data-status={task.status} data-focused={focused || undefined}>
      <div className="qzip-task-card__icon"><img src={taskFormatIcon(task)} alt={text(`${taskFormatLabel(task)} 文件图标`, `${taskFormatLabel(task)} file icon`)} /></div>
      <div className="qzip-task-card__body" id={`qzip-task-${task.taskId}`}>
        <div className="qzip-task-card__heading">
          <strong>{task.displayName}</strong>
          <span className={`qzip-status qzip-status--${task.status}`}>{status}</span>
          {active && task.progress?.percent != null ? <b>{percent}%</b> : null}
        </div>
        {active ? <Progress value={Math.max(percent, task.status === "queued" ? 4 : task.status === "scanning" ? 8 : 0)} /> : null}
        <div className="qzip-task-card__facts">
          <span><strong>{text("操作", "Operation")}</strong>{taskOperationLabel(task.operation, locale)}</span>
          <span><strong>{active ? text("当前阶段", "Current phase") : text("更新时间", "Updated")}</strong>{active ? phaseLabel : formatTaskTimestamp(task.updatedAt, locale)}</span>
          {task.output ? <span className="qzip-task-card__fact--wide"><strong>{outputLabel}</strong><span className="qzip-task-card__output">{task.output}</span></span> : null}
          {active && task.progress?.currentEntry ? <span className="qzip-task-card__fact--wide"><strong>{text("当前文件", "Current file")}</strong><span className="qzip-task-card__output">{task.progress.currentEntry}</span></span> : null}
          {active ? <span><strong>{text("已用时间", "Elapsed")}</strong>{formatElapsed(task.progress?.elapsedSeconds)}</span> : null}
          {task.error ? <span className="qzip-task-card__fact--wide qzip-task-card__fact--error"><strong>{text("失败原因", "Failure reason")}</strong>{task.error.message}</span> : null}
        </div>
        {detailsOpen && hasDetails ? (
          <div className="qzip-task-card__details">
            {task.error ? <span><strong>{text("错误代码", "Error code")}</strong>{task.error.code}</span> : null}
            {task.warnings.length ? <span><strong>{text("警告", "Warnings")}</strong>{task.warnings.join(text("；", "; "))}</span> : null}
          </div>
        ) : null}
        {needsPassword && showPassword ? <Input aria-label={text("重试密码", "Retry password")} type="password" value={password} onChange={(event) => setPassword(event.target.value)} placeholder={text("请输入正确密码", "Enter the correct password")} /> : null}
      </div>
      <div className="qzip-task-card__actions">
        {active ? <Button variant="icon" aria-label={text("暂停任务（暂不支持）", "Pause task (not supported)")} disabled title={text("当前版本暂不支持暂停任务", "Pausing tasks is not supported yet")} icon={<PauseRegular fontSize={22} />} /> : null}
        {active ? <Button variant="danger" icon={<DismissRegular fontSize={19} />} onClick={() => onCancel(task.taskId)}>{text("取消任务", "Cancel task")}</Button> : null}
        {task.status === "failed" && task.retryable ? (
          <Button
            variant="warning"
            icon={<WarningRegular fontSize={19} />}
            disabled={showPassword && needsPassword && !password}
            onClick={() => showPassword ? onRetry(task.taskId, password || undefined) : setShowPassword(true)}
          >
            {showPassword ? text("确认重试", "Retry") : needsPassword ? text("重新输入密码", "Enter password") : text("重试", "Retry")}
          </Button>
        ) : null}
        {hasDetails ? <Button variant="tertiary" icon={<MoreHorizontalRegular fontSize={19} />} onClick={() => setDetailsOpen((current) => !current)}>{detailsOpen ? text("收起详情", "Hide details") : text("查看详情", "View details")}</Button> : null}
        {task.status === "completed" && task.output ? <Button variant="primary" icon={<OpenRegular fontSize={19} />} onClick={() => void archiveClient.open(task.output!)}>{text("打开结果", "Open result")}</Button> : null}
        {task.output ? <Button variant="secondary" onClick={() => void archiveClient.reveal(task.output!)}>{text("打开位置", "Open location")}</Button> : null}
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
  const { text } = useI18n();
  return (
    <section className={`qzip-detail-page ${className ?? ""}`}>
      <Card className="qzip-detail-panel">
        <header className="qzip-detail-panel__header">
          <button type="button" aria-label={text("返回首页", "Back to home")} onClick={onBack}><ArrowLeftRegular fontSize={26} /></button>
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
