import { useEffect, useState } from "react";
import {
  type AccentTheme,
  type ThemeMode
} from "@qzip/ui";
import { Header } from "../components/Header";
import { TaskPopover } from "../components/TaskPopover";
import { ThemePopover } from "../components/ThemePopover";
import { Toast } from "../components/Toast";
import { HomePage } from "../features/home/HomePage";
import {
  resolveThemeMode,
  useAppearanceStore
} from "../stores/appearance";

type Popover = "tasks" | "theme" | null;

function getSystemDark(): boolean {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;
}

export function App() {
  const { mode, accent, setMode, setAccent } = useAppearanceStore();
  const [systemDark, setSystemDark] = useState(getSystemDark);
  const [popover, setPopover] = useState<Popover>(null);
  const [toast, setToast] = useState<string | null>(null);
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

  function togglePopover(next: Exclude<Popover, null>) {
    setPopover((current) => (current === next ? null : next));
  }

  function showUnavailable() {
    setToast("文件处理将在 M2 和 M3 里程碑接入。");
  }

  return (
    <main className="qzip-app-shell">
      <Header
        onTasksClick={() => togglePopover("tasks")}
        onSettingsClick={() => togglePopover("theme")}
      />
      <section className="qzip-app-content">
        <HomePage onUnavailable={showUnavailable} />
      </section>
      {popover === "tasks" ? (
        <TaskPopover onClose={() => setPopover(null)} />
      ) : null}
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
