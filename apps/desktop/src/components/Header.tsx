import {
  AppsListRegular,
  DismissRegular,
  MaximizeRegular,
  SettingsRegular,
  SubtractRegular
} from "@fluentui/react-icons";
import { Button } from "@qzip/ui";

interface HeaderProps {
  activePage?: string;
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

export function Header({ activePage, iconSrc, onHomeClick, onTasksClick, onSettingsClick }: HeaderProps) {
  return (
    <header className="qzip-titlebar">
      <button type="button" className="qzip-brand" aria-label="返回首页" onClick={onHomeClick}>
        <span className="qzip-brand__icon" aria-hidden="true">
          <img src={iconSrc} alt="" />
        </span>
        <span className="qzip-brand__name">轻压</span>
      </button>
      <div
        className="qzip-titlebar__drag-region"
        data-tauri-drag-region
        onDoubleClick={() => void withCurrentWindow((window) => window.toggleMaximize())}
      />
      <nav className="qzip-titlebar__actions" aria-label="应用导航">
        <Button
          variant="tertiary"
          className="qzip-titlebar__nav-button"
          data-active={activePage === "tasks"}
          icon={<AppsListRegular fontSize={22} />}
          onClick={onTasksClick}
        >
          任务
        </Button>
        <Button
          variant="tertiary"
          className="qzip-titlebar__nav-button"
          data-active={activePage === "settings"}
          icon={<SettingsRegular fontSize={22} />}
          onClick={onSettingsClick}
        >
          设置
        </Button>
      </nav>
      <span className="qzip-titlebar__separator" aria-hidden="true" />
      <div className="qzip-window-controls" aria-label="窗口控制">
        <button
          type="button"
          aria-label="最小化"
          onClick={() => void withCurrentWindow((window) => window.minimize())}
        >
          <SubtractRegular fontSize={18} />
        </button>
        <button
          type="button"
          aria-label="最大化或还原"
          onClick={() => void withCurrentWindow((window) => window.toggleMaximize())}
        >
          <MaximizeRegular fontSize={16} />
        </button>
        <button
          type="button"
          className="qzip-window-controls__close"
          aria-label="关闭"
          onClick={() => void withCurrentWindow((window) => window.close())}
        >
          <DismissRegular fontSize={19} />
        </button>
      </div>
    </header>
  );
}
