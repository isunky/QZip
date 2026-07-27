import { useEffect, useState } from "react";
import {
  type AccentTheme,
  type ThemeMode
} from "@qzip/ui";
import { Header } from "../components/Header";
import { ThemePopover } from "../components/ThemePopover";
import { Toast } from "../components/Toast";
import { HomePage } from "../features/home/HomePage";
import { BrowserPage, CreatePage, ExtractPage, TaskCenter, type Page } from "../features/archive/ArchivePages";
import type { ArchiveSession, TaskSnapshot } from "../contracts/archive";
import { archiveClient } from "../lib/archiveClient";
import {
  resolveThemeMode,
  useAppearanceStore
} from "../stores/appearance";

type Popover = "theme" | null;

const demoSession: ArchiveSession = {
  sessionId: "demo-session", format: "zip", compressedSize: 2_189_122,
  estimatedUncompressedSize: 4_383_462, entryCount: 5, encrypted: false, risks: []
};

function getSystemDark(): boolean {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;
}

export function App() {
  const { mode, accent, setMode, setAccent } = useAppearanceStore();
  const [systemDark, setSystemDark] = useState(getSystemDark);
  const [popover, setPopover] = useState<Popover>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [page, setPage] = useState<Page>("home");
  const [tasks, setTasks] = useState<TaskSnapshot[]>([]);
  const [archive, setArchive] = useState<string>("D:\\QZip\\示例压缩包.zip");
  const [session, setSession] = useState<ArchiveSession>(demoSession);
  const resolvedMode = resolveThemeMode(mode, systemDark);

  useEffect(() => {
    if (!window.matchMedia) {
      return;
    }

    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setSystemDark(media.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    document.documentElement.dataset.mode = resolvedMode;
    document.documentElement.dataset.accent = accent;
  }, [accent, resolvedMode]);

  useEffect(() => {
    if (!toast) {
      return;
    }

    const timer = window.setTimeout(() => setToast(null), 2800);
    return () => window.clearTimeout(timer);
  }, [toast]);

  useEffect(() => {
    if (!archiveClient.isTauri) {
      return;
    }
    void archiveClient.tasks().then(setTasks).catch((reason) => setToast(String(reason)));
    let stopped = false;
    let unlisten: (() => void) | undefined;
    void archiveClient.onTaskEvent((event) => {
      if (stopped) return;
      setTasks((current) => {
        const rest = current.filter((task) => task.taskId !== event.task.taskId);
        return [event.task, ...rest].sort((left, right) => right.updatedAt - left.updatedAt);
      });
    }).then((next) => { unlisten = next; });
    return () => { stopped = true; unlisten?.(); };
  }, []);

  function togglePopover(next: Exclude<Popover, null>) {
    setPopover((current) => (current === next ? null : next));
  }

  async function openArchive() {
    try {
      const selected = archiveClient.isTauri ? await archiveClient.pickInputPaths(true) : [archive];
      if (!selected[0]) return;
      const target = selected[0];
      setArchive(target);
      setSession(archiveClient.isTauri ? await archiveClient.prepare(target) : demoSession);
      setPage("extract");
    } catch (reason) { setToast(String(reason)); }
  }

  function addTask(task: TaskSnapshot) {
    setTasks((current) => [task, ...current.filter((item) => item.taskId !== task.taskId)]);
    setToast("任务已加入队列，可在任务中心查看进度。");
  }

  function goHome() { setPage("home"); }

  function currentPage() {
    if (page === "create") return <CreatePage onBack={goHome} onCreated={addTask} />;
    if (page === "extract") return <ExtractPage archive={archive} session={session} onBack={goHome} onBrowse={() => setPage("browser")} onCreated={addTask} />;
    if (page === "browser") return <BrowserPage archive={archive} session={session} onBack={() => setPage("extract")} onExtract={() => setPage("extract")} />;
    if (page === "tasks") return <TaskCenter tasks={tasks} onBack={goHome} onClear={() => { if (archiveClient.isTauri) void archiveClient.clearCompleted().then(() => setTasks((current) => current.filter((task) => !["completed", "failed", "cancelled"].includes(task.status)))); else setTasks((current) => current.filter((task) => !["completed", "failed", "cancelled"].includes(task.status))); }} onCancel={(taskId) => { if (archiveClient.isTauri) void archiveClient.cancel(taskId); else setTasks((current) => current.map((task) => task.taskId === taskId ? { ...task, status: "cancelled", updatedAt: Date.now() } : task)); }} onRetry={(taskId) => { if (archiveClient.isTauri) void archiveClient.retry(taskId).then(addTask); else setTasks((current) => current.map((task) => task.taskId === taskId ? { ...task, status: "queued", updatedAt: Date.now(), error: undefined } : task)); }} />;
    return <HomePage onCreate={() => setPage("create")} onOpenArchive={() => void openArchive()} />;
  }

  return (
    <main className="qzip-app-shell">
      <Header
        onTasksClick={() => setPage("tasks")}
        onSettingsClick={() => togglePopover("theme")}
      />
      <section className="qzip-app-content">
        {currentPage()}
      </section>
      {popover === "theme" ? (
        <ThemePopover
          mode={mode}
          accent={accent}
          onModeChange={(nextMode: ThemeMode) => setMode(nextMode)}
          onAccentChange={(nextAccent: AccentTheme) => setAccent(nextAccent)}
          onClose={() => setPopover(null)}
        />
      ) : null}
      {toast ? <Toast message={toast} onClose={() => setToast(null)} /> : null}
    </main>
  );
}
