import { ListTodo, X } from "lucide-react";
import { Button } from "@qzip/ui";
import { useI18n } from "../lib/i18n";

interface TaskPopoverProps {
  onClose: () => void;
}

export function TaskPopover({ onClose }: TaskPopoverProps) {
  const { text } = useI18n();
  return (
    <aside className="qzip-popover qzip-popover--tasks" aria-label={text("任务", "Tasks")}>
      <div className="qzip-popover__header">
        <strong>{text("任务", "Tasks")}</strong>
        <Button
          variant="icon"
          aria-label={text("关闭任务面板", "Close tasks panel")}
          title={text("关闭", "Close")}
          icon={<X size={18} />}
          onClick={onClose}
        />
      </div>
      <div className="qzip-empty-state">
        <span className="qzip-empty-state__icon" aria-hidden="true">
          <ListTodo size={25} />
        </span>
        <p>{text("暂无进行中的任务", "No active tasks")}</p>
        <span>{text("压缩和解压任务将在后续里程碑接入。", "Compression and extraction tasks will appear here.")}</span>
      </div>
    </aside>
  );
}
