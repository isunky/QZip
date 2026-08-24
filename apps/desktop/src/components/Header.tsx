import {
  AppsListRegular,
  DismissRegular,
  MaximizeRegular,
  SettingsRegular,
  SubtractRegular
} from "@fluentui/react-icons";
import { Button } from "@qzip/ui";
import { useI18n } from "../lib/i18n";

interface HeaderProps {
  activePage?: string;
  activeTaskCount?: number;
  iconSrc: string;
  onHomeClick: () => void;
  onTasksClick: () => void;
  onSettingsClick: () => void;
}

const isTauri = "__TAURI_INTERNALS__" in window;

async function withCurrentWindow(
  action: (window: Awaited<ReturnType<typeof import("@tauri-apps/api/window")["getCurrentWindow"]>>) => Promise<void>
) {
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await action(getCurrentWindow());
}

export function Header({ activePage, activeTaskCount = 0, iconSrc, onHomeClick, onTasksClick, onSettingsClick }: HeaderProps) {
  const { brandName, text } = useI18n();
  return (
    <header className="qzip-titlebar">
      <button type="button" className="qzip-brand" aria-label={text("返回首页", "Back to home")} onClick={onHomeClick}>
        <span className="qzip-brand__icon" aria-hidden="true">
          <img src={iconSrc} alt="" />
        </span>
        <span className="qzip-brand__name">{brandName}</span>
      </button>
      <div
        className="qzip-titlebar__drag-region"
        data-tauri-drag-region
        onDoubleClick={() => void withCurrentWindow((window) => window.toggleMaximize())}
      />
      <nav className="qzip-titlebar__actions" aria-label={text("应用导航", "App navigation")}>
        <Button
          variant="tertiary"
          className="qzip-titlebar__nav-button"
          data-active={activePage === "tasks"}
          icon={<AppsListRegular fontSize={22} />}
          onClick={onTasksClick}
        >
          {text("任务", "Tasks")}
          {activeTaskCount ? <span className="qzip-titlebar__task-count" aria-label={text(`${activeTaskCount} 个进行中任务`, `${activeTaskCount} active tasks`)}>{activeTaskCount}</span> : null}
        </Button>
        <Button
          variant="tertiary"
          className="qzip-titlebar__nav-button"
          data-active={activePage === "settings"}
          icon={<SettingsRegular fontSize={22} />}
          onClick={onSettingsClick}
        >
          {text("设置", "Settings")}
        </Button>
      </nav>
      <span className="qzip-titlebar__separator" aria-hidden="true" />
      <div className="qzip-window-controls" aria-label={text("窗口控制", "Window controls")}>
        <button
          type="button"
          aria-label={text("最小化", "Minimize")}
          onClick={() => void withCurrentWindow((window) => window.minimize())}
        >
          <SubtractRegular fontSize={18} />
        </button>
        <button
          type="button"
          aria-label={text("最大化或还原", "Maximize or restore")}
          onClick={() => void withCurrentWindow((window) => window.toggleMaximize())}
        >
          <MaximizeRegular fontSize={16} />
        </button>
        <button
          type="button"
          className="qzip-window-controls__close"
          aria-label={text("关闭", "Close")}
          onClick={() => void withCurrentWindow((window) => window.close())}
        >
          <DismissRegular fontSize={19} />
        </button>
      </div>
    </header>
  );
}
