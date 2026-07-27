import { useEffect, useMemo, useState } from "react";
import {
  Archive, ChevronRight, FileArchive, FileCheck2, FileInput, FolderOpen,
  History, LockKeyhole, MoreHorizontal, PackageOpen, Play, RefreshCw,
  Search, ShieldAlert, Trash2, X
} from "lucide-react";
import { Button, Card, Input, Progress, SegmentedControl } from "@qzip/ui";
import type {
  ArchiveEntry, ArchiveFormat, ArchiveRisk, ArchiveSession, CompressionProfile,
  ConflictPolicy, CreateTaskRequest, TaskSnapshot
} from "../../contracts/archive";
import { archiveClient } from "../../lib/archiveClient";

type Page = "home" | "create" | "extract" | "browser" | "tasks";

const formatOptions = [
  { value: "sevenZip", label: "7Z" }, { value: "zip", label: "ZIP" },
  { value: "tar", label: "TAR" }, { value: "tarGz", label: "TAR.GZ" }, { value: "tarXz", label: "TAR.XZ" }
] as const;
const profiles = [
  { value: "fast", label: "最快" }, { value: "balanced", label: "均衡" },
  { value: "small", label: "最小" }
] as const;

const demoEntries: ArchiveEntry[] = [
  { path: "资料/", displayName: "资料", size: 0, isDirectory: true, encrypted: false, isSymlink: false, isHardlink: false },
  { path: "资料/项目说明.pdf", displayName: "项目说明.pdf", size: 2_851_840, compressedSize: 1_204_000, isDirectory: false, encrypted: false, isSymlink: false, isHardlink: false },
  { path: "图片/", displayName: "图片", size: 0, isDirectory: true, encrypted: false, isSymlink: false, isHardlink: false },
  { path: "图片/封面.png", displayName: "封面.png", size: 1_523_200, compressedSize: 980_050, isDirectory: false, encrypted: false, isSymlink: false, isHardlink: false },
  { path: "readme.txt", displayName: "readme.txt", size: 8_422, compressedSize: 2_641, isDirectory: false, encrypted: false, isSymlink: false, isHardlink: false }
];

function formatBytes(value: number) {
  if (!value) return "—";
  const units = ["B", "KB", "MB", "GB"];
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${(value / 1024 ** index).toFixed(index ? 1 : 0)} ${units[index]}`;
}

function RiskNotice({ risks, accepted, onAccepted }: { risks: ArchiveRisk[]; accepted: boolean; onAccepted: (value: boolean) => void }) {
  if (!risks.length) return null;
  const mayContinue = risks.some((risk) => risk.overridable);
  return <aside className="qzip-risk-notice">
    <ShieldAlert size={20} />
    <div><strong>安全检查提示</strong><p>{risks.map((risk) => risk.message).join("；")}</p>
      {mayContinue ? <label><input type="checkbox" checked={accepted} onChange={(event) => onAccepted(event.target.checked)} /> 我已了解本次风险并继续</label> : null}
    </div>
  </aside>;
}

export function CreatePage({ onBack, onCreated, defaultFormat = "sevenZip", defaultProfile = "balanced", defaultTestAfterCreate = true, initialInputs = [] }: { onBack: () => void; onCreated: (task: TaskSnapshot) => void; defaultFormat?: ArchiveFormat; defaultProfile?: CompressionProfile; defaultTestAfterCreate?: boolean; initialInputs?: string[] }) {
  const [inputs, setInputs] = useState<string[]>([]);
  const [format, setFormat] = useState<ArchiveFormat>(defaultFormat);
  const [profile, setProfile] = useState<CompressionProfile>(defaultProfile);
  const [output, setOutput] = useState("D:\\QZip\\新建压缩包.7z");
  const [password, setPassword] = useState("");
  const [testing, setTesting] = useState(defaultTestAfterCreate);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { if (initialInputs.length) setInputs((current) => [...new Set([...current, ...initialInputs])]); }, [initialInputs]);
  const addFiles = async () => {
    if (!archiveClient.isTauri) { setInputs(["D:\\示例文件夹", "D:\\报价单.xlsx"]); return; }
    const picked = await archiveClient.pickInputPaths(false);
    setInputs((current) => [...new Set([...current, ...picked])]);
  };
  const start = async () => {
    setBusy(true); setError(null);
    try {
      const request: CreateTaskRequest = { inputs, output, format, profile, password: password || undefined, encryptHeaders: Boolean(password), testAfterCreate: testing, deleteSourcesAfterSuccess: false };
      if (!archiveClient.isTauri) {
        onCreated({ taskId: crypto.randomUUID(), operation: "create", status: "queued", displayName: output.split("\\").pop() ?? "新建压缩包.7z", output, createdAt: Date.now(), updatedAt: Date.now(), warnings: [], retryable: true });
      } else onCreated(await archiveClient.create(request));
      onBack();
    } catch (reason) { setError(String(reason)); } finally { setBusy(false); }
  };
  return <Workspace title="新建压缩包" subtitle="选择文件并配置压缩方案" onBack={onBack}>
    <div className="qzip-form-grid qzip-form-grid--create"><Card className="qzip-work-card"><Section title="要压缩的文件" action={<Button variant="secondary" icon={<FolderOpen size={18} />} onClick={() => void addFiles()}>添加文件</Button>}>
      <div className="qzip-path-list">{inputs.length ? inputs.map((path) => <span key={path}><FileInput size={16} />{path}<button onClick={() => setInputs((current) => current.filter((item) => item !== path))}><X size={15} /></button></span>) : <Empty icon={<FileArchive />} text="尚未选择文件或文件夹" />}</div>
    </Section></Card><Card className="qzip-work-card"><Section title="压缩设置">
      <SegmentedControl options={formatOptions} value={format} onValueChange={(value) => setFormat(value as ArchiveFormat)} ariaLabel="压缩格式" />
      <SegmentedControl options={profiles} value={profile} onValueChange={(value) => setProfile(value as CompressionProfile)} ariaLabel="压缩等级" />
      <Input label="保存位置与文件名" value={output} onChange={(event) => setOutput(event.target.value)} />
      <Input label="密码（可选）" type="password" value={password} onChange={(event) => setPassword(event.target.value)} hint="密码不会写入任务历史" />
      <label className="qzip-check"><input type="checkbox" checked={testing} onChange={(event) => setTesting(event.target.checked)} /> 创建完成后测试压缩包完整性</label>
      {error ? <p className="qzip-form-error">{error}</p> : null}
      <Button className="qzip-primary-action" loading={busy} disabled={!inputs.length || !output} icon={<Play size={18} />} onClick={() => void start()}>开始压缩</Button>
    </Section></Card></div>
  </Workspace>;
}

export function ExtractPage({ archive, session, onBack, onBrowse, onCreated, defaultConflictPolicy = "rename" }: { archive: string; session: ArchiveSession; onBack: () => void; onBrowse: () => void; onCreated: (task: TaskSnapshot) => void; defaultConflictPolicy?: ConflictPolicy }) {
  const [output, setOutput] = useState("D:\\QZip\\解压结果");
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>(defaultConflictPolicy);
  const [password, setPassword] = useState(""); const [accepted, setAccepted] = useState(false); const [busy, setBusy] = useState(false); const [error, setError] = useState<string | null>(null);
  const start = async () => { setBusy(true); setError(null); try { const request = { archive, output, conflictPolicy, password: password || undefined, acceptRisk: accepted }; if (!archiveClient.isTauri) onCreated({ taskId: crypto.randomUUID(), operation: "extract", status: "queued", displayName: archive.split("\\").pop() ?? "示例压缩包.zip", output, createdAt: Date.now(), updatedAt: Date.now(), warnings: [], retryable: true }); else onCreated(await archiveClient.extract(request)); onBack(); } catch (reason) { setError(String(reason)); } finally { setBusy(false); } };
  const hasBlocking = session.risks.some((risk) => !risk.overridable);
  return <Workspace title="快速解压" subtitle="先完成安全预检，再开始解压" onBack={onBack}>
    <div className="qzip-form-grid"><Card className="qzip-work-card"><Section title="待解压文件"><div className="qzip-archive-summary"><PackageOpen size={34} /><div><strong>{archive.split("\\").pop()}</strong><span>{session.entryCount} 项 · 预计 {formatBytes(session.estimatedUncompressedSize)}</span></div></div><Button variant="tertiary" onClick={onBrowse}>浏览内容并选择文件</Button></Section></Card>
      <Card className="qzip-work-card"><Section title="解压设置"><Input label="解压到" value={output} onChange={(event) => setOutput(event.target.value)} /><Input label="密码（如需要）" type="password" value={password} onChange={(event) => setPassword(event.target.value)} /><SegmentedControl options={[{ value: "rename", label: "自动重命名" }, { value: "overwrite", label: "覆盖" }, { value: "skip", label: "跳过" }]} value={conflictPolicy} onValueChange={(value) => setConflictPolicy(value as ConflictPolicy)} ariaLabel="文件冲突处理" /><RiskNotice risks={session.risks} accepted={accepted} onAccepted={setAccepted} />{error ? <p className="qzip-form-error">{error}</p> : null}<Button className="qzip-primary-action" loading={busy} disabled={hasBlocking || (session.risks.length > 0 && !accepted)} icon={<Play size={18} />} onClick={() => void start()}>开始解压</Button></Section></Card>
    </div>
  </Workspace>;
}

export function BrowserPage({ archive, session, onBack, onExtract }: { archive: string; session: ArchiveSession; onBack: () => void; onExtract: () => void }) {
  const [search, setSearch] = useState(""); const [entries, setEntries] = useState<ArchiveEntry[]>(archiveClient.isTauri ? [] : demoEntries); const [loading, setLoading] = useState(archiveClient.isTauri); const [selected, setSelected] = useState<Set<string>>(new Set());
  useEffect(() => {
    if (archiveClient.isTauri) {
      void archiveClient.entries(session.sessionId).then((page) => setEntries(page.entries)).finally(() => setLoading(false));
    }
  }, [session.sessionId]);
  const shown = useMemo(() => entries.filter((entry) => entry.displayName.toLowerCase().includes(search.toLowerCase())), [entries, search]);
  return <Workspace title={archive.split("\\").pop() ?? "压缩包内容"} subtitle={`${session.entryCount} 项内容 · 浏览不解压`} onBack={onBack}>
    <Card className="qzip-browser-card"><div className="qzip-browser-toolbar"><div className="qzip-breadcrumb"><Archive size={17} /> 压缩包 <ChevronRight size={15} /> 根目录</div><Input aria-label="搜索压缩包内容" placeholder="搜索文件名" value={search} onChange={(event) => setSearch(event.target.value)} trailing={<Search size={17} />} /><Button variant="secondary" icon={<PackageOpen size={18} />} onClick={onExtract}>解压所选{selected.size ? ` (${selected.size})` : ""}</Button></div><div className="qzip-entry-table" role="table"><div className="qzip-entry-table__head" role="row"><span>名称</span><span>大小</span><span>压缩后</span><span>修改时间</span></div>{loading ? <Empty icon={<RefreshCw className="qzip-spin" />} text="正在读取压缩包目录…" /> : shown.map((entry) => <button className="qzip-entry-row" key={entry.path} role="row" onClick={() => setSelected((current) => { const next = new Set(current); next.has(entry.path) ? next.delete(entry.path) : next.add(entry.path); return next; })} data-selected={selected.has(entry.path)}><span>{entry.isDirectory ? <FolderOpen size={18} /> : <FileArchive size={18} />}{entry.displayName}{entry.encrypted ? <LockKeyhole size={14} /> : null}</span><span>{entry.isDirectory ? "—" : formatBytes(entry.size)}</span><span>{entry.isDirectory ? "—" : formatBytes(entry.compressedSize ?? 0)}</span><span>{entry.modifiedAt?.slice(0, 10) ?? "—"}</span></button>)}{!shown.length && !loading ? <Empty icon={<Search />} text="没有匹配的文件" /> : null}</div></Card>
  </Workspace>;
}

export function TaskCenter({ tasks, onBack, onClear, onCancel, onRetry }: { tasks: TaskSnapshot[]; onBack: () => void; onClear: () => void; onCancel: (id: string) => void; onRetry: (id: string) => void }) {
  const [tab, setTab] = useState<"all" | "active" | "finished">("all");
  const shown = tasks.filter((task) => tab === "all" || tab === "active" ? !["completed", "failed", "cancelled"].includes(task.status) : ["completed", "failed", "cancelled"].includes(task.status));
  return <Workspace title="任务中心" subtitle="同时最多执行 2 个任务；保留最近 100 条本地记录" onBack={onBack} action={<Button variant="tertiary" icon={<Trash2 size={17} />} onClick={onClear}>清除已完成</Button>}>
    <div className="qzip-task-layout"><aside className="qzip-task-sidebar"><button data-active={tab === "all"} onClick={() => setTab("all")}><History size={18} />全部任务 <em>{tasks.length}</em></button><button data-active={tab === "active"} onClick={() => setTab("active")}><RefreshCw size={18} />进行中</button><button data-active={tab === "finished"} onClick={() => setTab("finished")}><FileCheck2 size={18} />已完成</button></aside><section className="qzip-task-list">{shown.length ? shown.map((task) => <TaskCard key={task.taskId} task={task} onCancel={onCancel} onRetry={onRetry} />) : <Card className="qzip-work-card"><Empty icon={<FileArchive />} text="这里还没有任务" /></Card>}</section></div>
  </Workspace>;
}

function TaskCard({ task, onCancel, onRetry }: { task: TaskSnapshot; onCancel: (id: string) => void; onRetry: (id: string) => void }) { const active = ["queued", "scanning", "running", "cancelling"].includes(task.status); return <Card className="qzip-task-card"><div className="qzip-task-card__icon"><FileArchive size={24} /></div><div className="qzip-task-card__body"><div><strong>{task.displayName}</strong><span className={`qzip-status qzip-status--${task.status}`}>{task.status === "completed" ? "已完成" : task.status === "failed" ? "失败" : task.status === "queued" ? "等待中" : "处理中"}</span></div><p>{task.progress?.currentEntry ?? task.progress?.phase ?? (task.error?.message || "准备处理")}</p>{active ? <Progress value={task.progress?.percent ?? 8} /> : null}</div><div className="qzip-task-card__actions">{active ? <Button variant="tertiary" title="暂停将在 7-Zip 支持安全挂起后开放" disabled>暂停</Button> : null}{active ? <Button variant="tertiary" onClick={() => onCancel(task.taskId)}>取消</Button> : null}{task.status === "failed" && task.retryable ? <Button variant="secondary" onClick={() => onRetry(task.taskId)}>重试</Button> : null}<Button variant="icon" aria-label="更多任务操作" icon={<MoreHorizontal size={19} />} /></div></Card>; }

function Workspace({ title, subtitle, onBack, action, children }: { title: string; subtitle: string; onBack: () => void; action?: React.ReactNode; children: React.ReactNode }) { return <section className="qzip-workspace"><div className="qzip-workspace__header"><button className="qzip-back" onClick={onBack}>← 返回首页</button><div><h1>{title}</h1><p>{subtitle}</p></div>{action}</div>{children}</section>; }
function Section({ title, action, children }: { title: string; action?: React.ReactNode; children: React.ReactNode }) { return <section className="qzip-form-section"><header><h2>{title}</h2>{action}</header>{children}</section>; }
function Empty({ icon, text }: { icon: React.ReactNode; text: string }) { return <div className="qzip-work-empty">{icon}<p>{text}</p></div>; }

export type { Page };
