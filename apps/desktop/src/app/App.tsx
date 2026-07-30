import { useCallback, useEffect, useRef, useState } from "react";
import type { AccentTheme, ThemeMode } from "@qzip/ui";
import { Header } from "../components/Header";
import { Toast } from "../components/Toast";
import { HomePage } from "../features/home/HomePage";
import { BrowserPage, CreatePage, ExtractPage, TaskCenter, type Page } from "../features/archive/ArchivePages";
import { SettingsPage } from "../features/settings/SettingsPage";
import type { ArchiveSession, TaskSnapshot } from "../contracts/archive";
import { defaultAppSettings, type AppSettings, uiScaleFactor } from "../contracts/settings";
import { archiveClient } from "../lib/archiveClient";
import { settingsClient } from "../lib/settingsClient";
import { resolveThemeMode, useAppearanceStore } from "../stores/appearance";

type AppPage = Page | "settings";
type ArchiveDestination = "browser" | "extract";
type PasswordPrompt = { archive: string; destination: ArchiveDestination; message: string };
const demoSession: ArchiveSession = { sessionId: "demo-session", format: "zip", compressedSize: 2_189_122, estimatedUncompressedSize: 4_383_462, entryCount: 5, encrypted: false, risks: [] };
const demoTasks: TaskSnapshot[] = [
  {
    taskId: "demo-active",
    operation: "create",
    status: "running",
    displayName: "项目资料.7z",
    output: "D:\\QZip\\项目资料.7z",
    createdAt: Date.now() - 86_000,
    updatedAt: Date.now(),
    progress: { phase: "正在压缩", percent: 68, currentEntry: "素材\\产品效果图.png", elapsedSeconds: 86 },
    warnings: [],
    retryable: false
  },
  {
    taskId: "demo-completed",
    operation: "extract",
    status: "completed",
    displayName: "设计交付.zip",
    output: "D:\\QZip\\设计交付",
    createdAt: Date.now() - 420_000,
    updatedAt: Date.now() - 305_000,
    progress: { phase: "已完成", percent: 100, elapsedSeconds: 115 },
    warnings: [],
    retryable: false
  },
  {
    taskId: "demo-failed",
    operation: "extract",
    status: "failed",
    displayName: "加密备份.7z",
    createdAt: Date.now() - 610_000,
    updatedAt: Date.now() - 606_000,
    error: { code: "WRONG_PASSWORD", message: "密码错误，请重新输入后重试。", recoverable: true },
    warnings: [],
    retryable: true
  }
];

function getSystemDark(): boolean { return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false; }

export function App() {
  const { mode, accent, setMode, setAccent } = useAppearanceStore();
  const [systemDark, setSystemDark] = useState(getSystemDark);
  const [toast, setToast] = useState<string | null>(null);
  const [page, setPage] = useState<AppPage>("home");
  const [tasks, setTasks] = useState<TaskSnapshot[]>(() => archiveClient.isTauri ? [] : demoTasks);
  const [archive, setArchive] = useState("D:\\QZip\\示例压缩包.zip");
  const [session, setSession] = useState<ArchiveSession>(demoSession);
  const [settings, setSettings] = useState<AppSettings>(defaultAppSettings);
  const [createInputs, setCreateInputs] = useState<string[]>([]);
  const [createFormat, setCreateFormat] = useState<"sevenZip" | "zip" | undefined>();
  const [selectedEntries, setSelectedEntries] = useState<string[]>([]);
  const [archivePassword, setArchivePassword] = useState("");
  const [passwordPrompt, setPasswordPrompt] = useState<PasswordPrompt | null>(null);
  const settingsRef = useRef(settings);
  const resolvedMode = resolveThemeMode(mode, systemDark);

  const applySettings = useCallback((next: AppSettings) => {
    settingsRef.current = next;
    setSettings(next);
    setMode(next.themeMode as ThemeMode);
    setAccent(next.accentTheme as AccentTheme);
  }, [setAccent, setMode]);

  useEffect(() => { void settingsClient.get().then(applySettings).catch(() => setToast("无法加载本机设置，已使用默认值。")); }, [applySettings]);
  useEffect(() => {
    if (!window.matchMedia) return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setSystemDark(media.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  const prepareArchive = useCallback(async (target: string, destination: ArchiveDestination, password?: string) => {
    try {
      const next = archiveClient.isTauri ? await archiveClient.prepare(target, password) : demoSession;
      setArchive(target);
      setSession(next);
      setArchivePassword(password ?? "");
      setPasswordPrompt(null);
      setSelectedEntries([]);
      setPage(destination);
    } catch (reason) {
      const message = String(reason);
      if (archiveClient.isTauri) {
        setPasswordPrompt({
          archive: target,
          destination,
          message: password ? `无法读取压缩包：${message}` : "压缩包可能已加密。请输入密码后重试；如未加密，请检查压缩包是否完整。"
        });
      } else {
        setToast(message);
      }
    }
  }, []);
  useEffect(() => {
    document.documentElement.dataset.mode = resolvedMode;
    document.documentElement.dataset.accent = accent;
    document.documentElement.dataset.density = settings.listDensity;
    document.documentElement.dataset.reduceMotion = String(settings.reduceMotion);
    if (settingsClient.isTauri) {
      void import("@tauri-apps/api/webview").then(({ getCurrentWebview }) => getCurrentWebview().setZoom(uiScaleFactor[settings.uiScale])).catch(() => undefined);
    }
  }, [accent, resolvedMode, settings.listDensity, settings.reduceMotion, settings.uiScale]);
  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(null), 2800);
    return () => window.clearTimeout(timer);
  }, [toast]);
  useEffect(() => {
    if (!archiveClient.isTauri) return;
    void archiveClient.tasks().then(setTasks).catch((reason) => setToast(String(reason)));
    let stopped = false; let unlisten: (() => void) | undefined;
    void archiveClient.onTaskEvent((event) => {
      if (stopped) return;
      setTasks((current) => [event.task, ...current.filter((task) => task.taskId !== event.task.taskId)].sort((left, right) => right.updatedAt - left.updatedAt));
      if (event.task.status === "completed") {
        setToast(event.task.operation === "create" ? "压缩文件已生成。" : "任务已完成。");
      } else if (event.task.status === "failed") {
        setToast(`任务失败：${event.task.error?.message ?? "请在任务中心查看详情"}`);
      }
      const currentSettings = settingsRef.current;
      const terminal = event.task.status === "completed" || event.task.status === "failed";
      const shouldNotify = terminal && currentSettings.taskNotificationsEnabled && (event.task.status === "completed" ? currentSettings.notifyOnSuccess : currentSettings.notifyOnFailure);
      if (shouldNotify) {
        void import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
          if (!(await getCurrentWindow().isFocused())) {
            const { sendNotification } = await import("@tauri-apps/plugin-notification");
            sendNotification({ title: event.task.status === "completed" ? "QZip 任务已完成" : "QZip 任务失败", body: event.task.status === "completed" ? "可在任务中心查看结果。" : "请在任务中心查看错误信息。" });
          }
        }).catch(() => undefined);
      }
    }).then((next) => { unlisten = next; });
    return () => { stopped = true; unlisten?.(); };
  }, []);
  useEffect(() => {
    if (!archiveClient.isTauri) return;
    let unlisten: (() => void) | undefined;
    const handleLaunchRequest = (request: { kind: string; paths: string[]; source: string }) => {
      const target = request.paths[0];
      if (!target) return;
      if (request.kind === "open") {
        void prepareArchive(target, "browser");
      } else if (request.kind === "compressSevenZip" || request.kind === "compressZip") {
        const format = request.kind === "compressZip" ? "zip" : "sevenZip";
        void archiveClient.suggestCreateOutput(request.paths, format)
          .then((output) => archiveClient.create({
            inputs: request.paths,
            output,
            format,
            profile: settingsRef.current.compressionProfile,
            encryptHeaders: false,
            testAfterCreate: settingsRef.current.testAfterCreate,
            deleteSourcesAfterSuccess: false
          }))
          .then((task) => {
            setTasks((current) => [task, ...current.filter((item) => item.taskId !== task.taskId)]);
            setPage("tasks");
            setToast("已开始执行右键压缩任务。");
          })
          .catch((reason) => setToast(`无法启动右键压缩：${String(reason)}`));
      } else if (request.kind === "moreOptions") {
        setCreateInputs(request.paths);
        setCreateFormat(undefined);
        setPage("create");
      } else if (request.kind === "extractHere" || request.kind === "extractNamed") {
        const named = request.kind === "extractNamed";
        void archiveClient.suggestExtractOutput(target, named)
          .then((output) => archiveClient.extract({
            archive: target,
            output,
            conflictPolicy: settingsRef.current.conflictPolicy,
            acceptRisk: false
          }))
          .then((task) => {
            setTasks((current) => [task, ...current.filter((item) => item.taskId !== task.taskId)]);
            setPage("tasks");
            setToast(named ? "已开始解压到同名文件夹。" : "已开始解压到此处。");
          })
          .catch((reason) => setToast(`无法启动右键解压：${String(reason)}`));
      }
    };
    void archiveClient.onLaunchRequest(handleLaunchRequest).then((next) => { unlisten = next; });
    void archiveClient.takeInitialLaunchRequest().then((request) => { if (request) handleLaunchRequest(request); }).catch(() => undefined);
    let checkingPendingRequest = false;
    const checkPendingRequest = () => {
      if (checkingPendingRequest) return;
      checkingPendingRequest = true;
      void archiveClient.takePendingShellRequest()
        .then((request) => { if (request) handleLaunchRequest(request); })
        .catch(() => undefined)
        .finally(() => { checkingPendingRequest = false; });
    };
    checkPendingRequest();
    const pendingRequestTimer = window.setInterval(checkPendingRequest, 750);
    return () => { window.clearInterval(pendingRequestTimer); unlisten?.(); };
  }, [prepareArchive]);

  async function openArchive() {
    try {
      const selected = archiveClient.isTauri ? await archiveClient.pickInputPaths(true) : [archive];
      if (!selected[0]) return;
      await prepareArchive(selected[0], "extract");
    } catch (reason) { setToast(String(reason)); }
  }
  function addTask(task: TaskSnapshot) { setTasks((current) => [task, ...current.filter((item) => item.taskId !== task.taskId)]); setToast("任务已加入队列，可在任务中心查看进度。"); }
  function goHome() { setArchivePassword(""); setPage("home"); }
  function currentPage() {
    if (page === "settings") return <SettingsPage settings={settings} onBack={goHome} onChanged={applySettings} onToast={setToast} />;
    if (page === "create") return <CreatePage onBack={goHome} onCreated={addTask} onOpenTasks={() => setPage("tasks")} defaultFormat={createFormat ?? settings.defaultFormat} defaultProfile={settings.compressionProfile} defaultTestAfterCreate={settings.testAfterCreate} initialInputs={createInputs} />;
    if (page === "extract") return <ExtractPage archive={archive} session={session} selectedEntries={selectedEntries} onBack={goHome} onBrowse={() => setPage("browser")} onCreated={(task) => { setArchivePassword(""); addTask(task); }} defaultConflictPolicy={settings.conflictPolicy} initialPassword={archivePassword} />;
    if (page === "browser") return <BrowserPage archive={archive} session={session} onBack={() => setPage("extract")} onClose={goHome} onExtract={(entries) => { setSelectedEntries(entries ?? []); setPage("extract"); }} onCreated={addTask} />;
    if (page === "tasks") return <TaskCenter tasks={tasks} onBack={goHome} onClear={() => { if (archiveClient.isTauri) void archiveClient.clearCompleted().then(() => setTasks((current) => current.filter((task) => !["completed", "failed", "cancelled"].includes(task.status)))).catch((reason) => setToast(`无法清理任务：${String(reason)}`)); else setTasks((current) => current.filter((task) => !["completed", "failed", "cancelled"].includes(task.status))); }} onCancel={(taskId) => { if (archiveClient.isTauri) void archiveClient.cancel(taskId).catch((reason) => setToast(`无法取消任务：${String(reason)}`)); else setTasks((current) => current.map((task) => task.taskId === taskId ? { ...task, status: "cancelled", updatedAt: Date.now() } : task)); }} onRetry={(taskId, password) => { if (archiveClient.isTauri) void archiveClient.retry(taskId, password).then(addTask).catch((reason) => setToast(`无法重试任务：${String(reason)}`)); else setTasks((current) => current.map((task) => task.taskId === taskId ? { ...task, status: "queued", updatedAt: Date.now(), error: undefined } : task)); }} />;
    return <HomePage onCreate={() => { setCreateInputs([]); setCreateFormat(undefined); setPage("create"); }} onOpenArchive={() => void openArchive()} />;
  }
  return <main className="qzip-app-shell"><Header activePage={page} onHomeClick={goHome} onTasksClick={() => setPage("tasks")} onSettingsClick={() => setPage("settings")} /><section className="qzip-app-content" data-page={page}>{currentPage()}</section>{passwordPrompt ? <section className="qzip-password-prompt" role="dialog" aria-modal="true" aria-labelledby="qzip-password-prompt-title"><form onSubmit={(event) => { event.preventDefault(); const data = new FormData(event.currentTarget); const password = String(data.get("password") ?? ""); if (!password) return; void prepareArchive(passwordPrompt.archive, passwordPrompt.destination, password); }}><h2 id="qzip-password-prompt-title">需要压缩包密码</h2><p>{passwordPrompt.message}</p><input name="password" type="password" autoFocus placeholder="请输入密码" aria-label="压缩包密码" /><div><button type="button" onClick={() => setPasswordPrompt(null)}>取消</button><button type="submit">继续</button></div></form></section> : null}{toast ? <Toast message={toast} onClose={() => setToast(null)} /> : null}</main>;
}
