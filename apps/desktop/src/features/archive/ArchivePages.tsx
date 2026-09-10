import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArchiveRegular,
  ArrowDownloadRegular,
  ChevronDownRegular,
  DismissRegular,
  DocumentAddRegular,
  DocumentRegular,
  EyeRegular,
  FolderAddRegular,
  FolderOpenRegular,
  FolderRegular,
  LockClosedRegular,
  ShieldCheckmarkRegular
} from "@fluentui/react-icons";
import { Button, Input, SegmentedControl } from "@qzip/ui";
import type {
  ArchiveFormat,
  ArchiveSession,
  BackendCapabilities,
  CompressionProfile,
  ConflictPolicy,
  CreateTaskRequest,
  TaskSnapshot
} from "../../contracts/archive";
import { archiveClient } from "../../lib/archiveClient";
import { useI18n } from "../../lib/i18n";
import { joinOutputPath, splitOutputPath, suggestCreateOutputLocally } from "./archivePath";
import {
  advancedFormatOptions,
  DetailWorkspace,
  errorMessage,
  formatBytes,
  formatLabels,
  fileName,
  FormRow,
  makeDemoTask,
  primaryFormatOptions,
  RiskNotice,
  Stat,
  type Page
} from "./ArchiveShared";
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
      let prepared: ArchiveSession | undefined;
      try {
        prepared = await archiveClient.prepare(target);
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
      } finally {
        if (prepared) await archiveClient.close(prepared.sessionId).catch(() => undefined);
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

export { BrowserPage } from "./BrowserPage";
export { TaskCenter } from "./TaskCenter";
export type { Page } from "./ArchiveShared";
