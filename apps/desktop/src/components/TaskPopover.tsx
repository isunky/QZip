import { ListTodo, X } from "lucide-react";
import { Button } from "@qzip/ui";

interface TaskPopoverProps {
  onClose: () => void;
}

export function TaskPopover({ onClose }: TaskPopoverProps) {
  return (
    <aside className="qzip-popover qzip-popover--tasks" aria-label="任务">
      <div className="qzip-popover__header">
        <strong>任务</strong>
        <Button
          variant="icon"
          aria-label="关闭任务面板"
          title="关闭"
          icon={<X size={18} />}
          onClick={onClose}
        />
      </div>
      <div className="qzip-empty-state">
        <span className="qzip-empty-state__icon" aria-hidden="true">
          <ListTodo size={25} />
        </span>
        <p>暂无进行中的任务</p>
        <span>压缩和解压任务将在后续里程碑接入。</span>
      </div>
    </aside>
  );
}
