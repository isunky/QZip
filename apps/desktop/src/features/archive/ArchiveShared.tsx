import type { ReactNode } from "react";
import {
  ArchiveRegular,
  ArrowLeftRegular,
  WarningRegular
} from "@fluentui/react-icons";
import { Card } from "@qzip/ui";
import type { ArchiveEntry, ArchiveFormat, ArchiveRisk, TaskSnapshot } from "../../contracts/archive";
import { localize, useI18n, type AppLocale } from "../../lib/i18n";

export type Page = "home" | "create" | "extract" | "batchExtract" | "browser" | "tasks";
export type EntrySortKey = "name" | "size" | "type" | "modified";
export type SortDirection = "ascending" | "descending";

export const primaryFormatOptions = [
  { value: "sevenZip", label: "7Z" },
  { value: "zip", label: "ZIP" },
  { value: "tar", label: "TAR" }
] as const;
export const advancedFormatOptions = [
  { value: "tarGz", label: "TAR.GZ" },
  { value: "tarXz", label: "TAR.XZ" }
] as const;
export const demoEntries: ArchiveEntry[] = [
  { path: "设计资料/", displayName: "设计资料", size: 0, isDirectory: true, modifiedAt: "2024-05-20T10:24:00", encrypted: false, isSymlink: false, isHardlink: false },
  { path: "项目文档/", displayName: "项目文档", size: 0, isDirectory: true, modifiedAt: "2024-05-18T09:15:00", encrypted: false, isSymlink: false, isHardlink: false },
  { path: "需求说明.md", displayName: "需求说明.md", size: 159_744, compressedSize: 58_400, isDirectory: false, modifiedAt: "2024-05-20T10:24:00", encrypted: false, isSymlink: false, isHardlink: false },
  { path: "项目预算.xlsx", displayName: "项目预算.xlsx", size: 49_869, compressedSize: 31_220, isDirectory: false, modifiedAt: "2024-05-19T16:42:00", encrypted: false, isSymlink: false, isHardlink: false },
  { path: "原型文件.fig", displayName: "原型文件.fig", size: 12_897_485, compressedSize: 4_287_800, isDirectory: false, modifiedAt: "2024-05-18T14:08:00", encrypted: false, isSymlink: false, isHardlink: false }
];

export const formatLabels: Record<ArchiveFormat, string> = {
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

export function formatBytes(value: number) {
  if (!value) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${(value / 1024 ** index).toFixed(index ? 1 : 0)} ${units[index]}`;
}
export function fileName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

export function fileExtension(path: string) {
  const name = fileName(path);
  const dot = name.lastIndexOf(".");
  return dot > 0 && dot < name.length - 1 ? name.slice(dot + 1).toLowerCase() : "";
}

export function errorMessage(reason: unknown) {
  if (reason && typeof reason === "object" && "message" in reason && typeof reason.message === "string") return reason.message;
  return String(reason);
}

export function formatType(entry: ArchiveEntry, locale: AppLocale) {
  if (entry.isDirectory) return localize(locale, "文件夹", "Folder");
  const extension = entry.displayName.split(".").pop()?.toUpperCase();
  return extension ? localize(locale, `${extension} 文件`, `${extension} file`) : localize(locale, "文件", "File");
}

export function formatElapsed(seconds = 0) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = Math.floor(seconds % 60);
  return [hours, minutes, rest].map((value) => String(value).padStart(2, "0")).join(":");
}

export function formatTaskTimestamp(value: number, locale: AppLocale) {
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

export function taskFormatInfo(task: TaskSnapshot) {
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

export function taskFormatLabel(task: TaskSnapshot) {
  return taskFormatInfo(task)?.[1] ?? "—";
}

export function taskFormatIcon(task: TaskSnapshot) {
  return `/file-types/${taskFormatInfo(task)?.[2] ?? "archive"}.ico`;
}

export function isActiveTask(task: TaskSnapshot) {
  return ["queued", "scanning", "running", "cancelling"].includes(task.status);
}

export function taskOperationLabel(operation: TaskSnapshot["operation"], locale: AppLocale) {
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

export function makeDemoTask(operation: "create" | "extract" | "test" | "update", name: string, output?: string): TaskSnapshot {
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

export function RiskNotice({ risks, accepted, onAccepted }: {
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

export function DetailWorkspace({ title, onBack, className, children }: {
  title: string;
  onBack: () => void;
  className?: string;
  children: ReactNode;
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

export function FormRow({ label, children }: { label: string; children: ReactNode }) {
  return <div className="qzip-form-row"><strong>{label}</strong><div>{children}</div></div>;
}

export function Stat({ label, value }: { label: string; value: string }) {
  return <div className="qzip-stat"><span>{label}</span><strong>{value}</strong></div>;
}

export function Empty({ icon, text }: { icon: ReactNode; text: string }) {
  return <div className="qzip-work-empty">{icon}<p>{text}</p></div>;
}
