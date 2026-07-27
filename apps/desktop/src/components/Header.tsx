import { useCallback } from "react";
import {
  FileArchive,
  ListTodo,
  Minus,
  Settings,
  Square,
  X
} from "lucide-react";
import { Button } from "@qzip/ui";

interface HeaderProps {
  onTasksClick: () => void;
  onSettingsClick: () => void;
}

type WindowAction = "minimize" | "maximize" | "close";

async function performWindowAction(action: WindowAction) {
  const isTauri = "__TAURI_INTERNALS__" in window;
  if (!isTauri) {
    return;
  }

  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const currentWindow = getCurrentWindow();

  if (action === "minimize") {
    await currentWindow.minimize();
  } else if (action === "maximize") {
    await currentWindow.toggleMaximize();
  } else {
    await currentWindow.close();
  }
}

export function Header({ onTasksClick, onSettingsClick }: HeaderProps) {
  const onWindowAction = useCallback((action: WindowAction) => {
    void performWindowAction(action);
  }, []);

  return (
    <header className="qzip-titlebar" data-tauri-drag-region>
      <div className="qzip-brand" data-tauri-drag-region>
        <span className="qzip-brand__icon" aria-hidden="true">
          <FileArchive size={22} strokeWidth={2.2} />
        </span>
        <span className="qzip-brand__name">轻压</span>
      </div>
      <nav className="qzip-titlebar__actions" aria-label="应用导航">
        <Button
          variant="tertiary"
          className="qzip-titlebar__nav-button"
          icon={<ListTodo size={20} />}
          onClick={onTasksClick}
        >
          任务
        </Button>
        <Button
          variant="tertiary"
          className="qzip-titlebar__nav-button"
          icon={<Settings size={20} />}
          onClick={onSettingsClick}
        >
          设置
        </Button>
        <span className="qzip-titlebar__divider" aria-hidden="true" />
        <Button
          variant="icon"
          aria-label="最小化窗口"
          title="最小化"
          onClick={() => onWindowAction("minimize")}
          icon={<Minus size={19} />}
        />
        <Button
          variant="icon"
          aria-label="最大化或还原窗口"
          title="最大化或还原"
          onClick={() => onWindowAction("maximize")}
          icon={<Square size={16} />}
        />
        <Button
          variant="icon"
          aria-label="关闭窗口"
          title="关闭"
          onClick={() => onWindowAction("close")}
          icon={<X size={20} />}
        />
      </nav>
    </header>
  );
}
