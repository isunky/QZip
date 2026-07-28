import {
  FileArchive,
  ListTodo,
  Settings
} from "lucide-react";
import { Button } from "@qzip/ui";

interface HeaderProps {
  onTasksClick: () => void;
  onSettingsClick: () => void;
}

export function Header({ onTasksClick, onSettingsClick }: HeaderProps) {
  return (
    <header className="qzip-titlebar">
      <div className="qzip-brand">
        <span className="qzip-brand__icon" aria-hidden="true">
          <FileArchive size={22} strokeWidth={2.2} />
        </span>
        <span className="qzip-brand__name">轻压</span>
      </div>
      <div className="qzip-titlebar__drag-region" aria-hidden="true" />
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
      </nav>
    </header>
  );
}
