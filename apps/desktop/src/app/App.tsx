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
        void archiveClient.prepare(target).then((next) => { setArchive(target); setSession(next); setPage("browser"); }).catch((reason) => setToast(String(reason)));
      } else if (request.kind === "compressSevenZip" || request.kind === "compressZip" || request.kind === "moreOptions") {
        setCreateInputs(request.paths);
        setCreateFormat(request.kind === "compressZip" ? "zip" : request.kind === "compressSevenZip" ? "sevenZip" : undefined);
        setPage("create");
      } else if (request.kind === "extractHere" || request.kind === "extractNamed") {
        void archiveClient.prepare(target).then((next) => { setArchive(target); setSession(next); setPage("extract"); }).catch((reason) => setToast(String(reason)));
      }
    };
    void archiveClient.onLaunchRequest(handleLaunchRequest).then((next) => { unlisten = next; });
    void archiveClient.takeInitialLaunchRequest().then((request) => { if (request) handleLaunchRequest(request); }).catch(() => undefined);
    return () => unlisten?.();
  }, []);

  async function openArchive() {
    try {
      const selected = archiveClient.isTauri ? await archiveClient.pickInputPaths(true) : [archive];
      if (!selected[0]) return;
      const target = selected[0]; setArchive(target);
      setSession(archiveClient.isTauri ? await archiveClient.prepare(target) : demoSession);
      setSelectedEntries([]);
      setPage("extract");
    } catch (reason) { setToast(String(reason)); }
  }
  function addTask(task: TaskSnapshot) { setTasks((current) => [task, ...current.filter((item) => item.taskId !== task.taskId)]); setToast("任务已加入队列，可在任务中心查看进度。"); }
  function goHome() { setPage("home"); }
  function currentPage() {
    if (page === "settings") return <SettingsPage settings={settings} onBack={goHome} onChanged={applySettings} onToast={setToast} />;
    if (page === "create") return <CreatePage onBack={goHome} onCreated={addTask} defaultFormat={createFormat ?? settings.defaultFormat} defaultProfile={settings.compressionProfile} defaultTestAfterCreate={settings.testAfterCreate} initialInputs={createInputs} />;
    if (page === "extract") return <ExtractPage archive={archive} session={session} selectedEntries={selectedEntries} onBack={goHome} onBrowse={() => setPage("browser")} onCreated={addTask} defaultConflictPolicy={settings.conflictPolicy} />;
    if (page === "browser") return <BrowserPage archive={archive} session={session} onBack={() => setPage("extract")} onClose={goHome} onExtract={(entries) => { setSelectedEntries(entries ?? []); setPage("extract"); }} onCreated={addTask} />;
    if (page === "tasks") return <TaskCenter tasks={tasks} onBack={goHome} onClear={() => { if (archiveClient.isTauri) void archiveClient.clearCompleted().then(() => setTasks((current) => current.filter((task) => !["completed", "failed", "cancelled"].includes(task.status)))); else setTasks((current) => current.filter((task) => !["completed", "failed", "cancelled"].includes(task.status))); }} onCancel={(taskId) => { if (archiveClient.isTauri) void archiveClient.cancel(taskId); else setTasks((current) => current.map((task) => task.taskId === taskId ? { ...task, status: "cancelled", updatedAt: Date.now() } : task)); }} onRetry={(taskId, password) => { if (archiveClient.isTauri) void archiveClient.retry(taskId, password).then(addTask); else setTasks((current) => current.map((task) => task.taskId === taskId ? { ...task, status: "queued", updatedAt: Date.now(), error: undefined } : task)); }} />;
    return <HomePage onCreate={() => { setCreateInputs([]); setCreateFormat(undefined); setPage("create"); }} onOpenArchive={() => void openArchive()} />;
  }
  return <main className="qzip-app-shell"><Header activePage={page} onHomeClick={goHome} onTasksClick={() => setPage("tasks")} onSettingsClick={() => setPage("settings")} /><section className="qzip-app-content" data-page={page}>{currentPage()}</section>{toast ? <Toast message={toast} onClose={() => setToast(null)} /> : null}</main>;
}
