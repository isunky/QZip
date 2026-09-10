import { useState } from "react";
import {
  ArchiveRegular,
  DeleteRegular,
  DismissRegular,
  HomeRegular,
  MoreHorizontalRegular,
  OpenRegular,
  PauseRegular,
  WarningRegular
} from "@fluentui/react-icons";
import { Button, Card, Input, Progress } from "@qzip/ui";
import type { TaskSnapshot } from "../../contracts/archive";
import { archiveClient } from "../../lib/archiveClient";
import { useI18n } from "../../lib/i18n";
import {
  Empty,
  formatElapsed,
  formatTaskTimestamp,
  isActiveTask,
  taskFormatIcon,
  taskFormatLabel,
  taskOperationLabel
} from "./ArchiveShared";

export function TaskCenter({
  tasks,
  focusTaskId,
  onBack,
  onClear,
  onCancel,
  onRetry
}: {
  tasks: TaskSnapshot[];
  focusTaskId?: string;
  onBack: () => void;
  onClear: () => void;
  onCancel: (id: string) => void;
  onRetry: (id: string, password?: string) => void;
}) {
  const { text } = useI18n();
  const active = tasks.filter(isActiveTask);
  const orderedTasks = [...tasks].sort((left, right) => {
    const activeDifference = Number(isActiveTask(right)) - Number(isActiveTask(left));
    if (activeDifference) return activeDifference;
    return right.updatedAt - left.updatedAt || right.createdAt - left.createdAt || right.taskId.localeCompare(left.taskId);
  });

  return (
    <section className="qzip-task-center qzip-task-center--single-list">
      <section className="qzip-task-content">
        <header className="qzip-task-content__header">
          <div>
            <h1>{text("任务中心", "Task center")}</h1>
            <span>{text(`共 ${tasks.length} 个任务`, `${tasks.length} tasks total`)}</span>
          </div>
          <div className="qzip-task-content__toolbar">
            <Button variant="tertiary" icon={<HomeRegular fontSize={19} />} onClick={onBack}>{text("返回首页", "Back to home")}</Button>
            {active.length ? <Button variant="danger" icon={<DismissRegular fontSize={19} />} onClick={() => active.forEach((task) => onCancel(task.taskId))}>{text("取消全部进行中", "Cancel active tasks")}</Button> : null}
            <Button variant="secondary" icon={<DeleteRegular fontSize={19} />} onClick={onClear}>{text("清空已结束", "Clear ended tasks")}</Button>
          </div>
        </header>
        <div className="qzip-task-list">
          {orderedTasks.length
            ? orderedTasks.map((task) => <TaskCard key={task.taskId} task={task} focused={task.taskId === focusTaskId} onCancel={onCancel} onRetry={onRetry} />)
            : <Card className="qzip-task-empty"><Empty icon={<ArchiveRegular fontSize={40} />} text={text("暂无任务记录", "No tasks yet")} /></Card>}
        </div>
      </section>
    </section>
  );
}
function TaskCard({
  task,
  focused,
  onCancel,
  onRetry
}: {
  task: TaskSnapshot;
  focused: boolean;
  onCancel: (id: string) => void;
  onRetry: (id: string, password?: string) => void;
}) {
  const { locale, text } = useI18n();
  const active = isActiveTask(task);
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const needsPassword = task.error?.code === "WRONG_PASSWORD";
  const percent = task.progress?.percent ?? (task.status === "completed" ? 100 : 0);
  const phase = task.progress?.phase;
  const phaseLabel = task.status === "queued"
    ? text("等待开始", "Waiting to start")
    : task.status === "scanning" || phase === "scanning"
      ? text("正在检查压缩包", "Checking archive")
      : task.status === "cancelling"
        ? text("正在取消", "Cancelling")
        : phase === "committing"
          ? text("正在整理结果", "Finalizing result")
          : phase === "creating"
            ? text("正在创建压缩包", "Creating archive")
            : phase === "testing"
              ? text("正在测试压缩包", "Testing archive")
              : phase === "updating"
                ? text("正在更新压缩包", "Updating archive")
                : task.operation === "extract"
                  ? text("正在解压", "Extracting")
                  : text("正在压缩", "Compressing");
  const statusLabel = task.status === "completed"
    ? text("已完成", "Completed")
    : task.status === "failed"
      ? needsPassword
        ? text("密码错误", "Password error")
        : text("失败", "Failed")
      : task.status === "cancelled"
        ? text("已取消", "Cancelled")
        : task.status === "queued"
          ? text("等待中", "Waiting")
          : task.operation === "extract"
            ? text("解压中", "Extracting")
            : text("压缩中", "Compressing");
  const outputLabel = task.operation === "extract"
    ? text("解压位置", "Extracted to")
    : task.operation === "create"
      ? text("压缩包位置", "Archive saved to")
      : text("目标位置", "Destination");
  const hasDetails = Boolean(task.error || task.warnings.length);

  return (
    <Card className="qzip-task-card" data-status={task.status} data-focused={focused || undefined}>
      <div className="qzip-task-card__identity">
        <img className="qzip-task-card__icon" src={taskFormatIcon(task)} alt={text(`${taskFormatLabel(task)} 文件图标`, `${taskFormatLabel(task)} file icon`)} />
        <span className="qzip-task-card__status">{statusLabel}</span>
      </div>
      <div className="qzip-task-card__body" id={`qzip-task-${task.taskId}`}>
        <div className="qzip-task-card__heading">
          <strong>{task.displayName}</strong>
          {active && task.progress?.percent != null ? <b>{percent}%</b> : null}
        </div>
        {active ? <Progress value={Math.max(percent, task.status === "queued" ? 4 : task.status === "scanning" ? 8 : 0)} /> : null}
        <div className="qzip-task-card__facts">
          <span><strong>{text("操作", "Operation")}</strong>{taskOperationLabel(task.operation, locale)}</span>
          <span><strong>{active ? text("当前阶段", "Current phase") : text("更新时间", "Updated")}</strong>{active ? phaseLabel : formatTaskTimestamp(task.updatedAt, locale)}</span>
          {task.output ? <span className="qzip-task-card__fact--wide"><strong>{outputLabel}</strong><span className="qzip-task-card__output">{task.output}</span></span> : null}
          {active && task.progress?.currentEntry ? <span className="qzip-task-card__fact--wide"><strong>{text("当前文件", "Current file")}</strong><span className="qzip-task-card__output">{task.progress.currentEntry}</span></span> : null}
          {active ? <span><strong>{text("已用时间", "Elapsed")}</strong>{formatElapsed(task.progress?.elapsedSeconds)}</span> : null}
          {task.error ? <span className="qzip-task-card__fact--wide qzip-task-card__fact--error"><strong>{text("失败原因", "Failure reason")}</strong>{task.error.message}</span> : null}
        </div>
        {detailsOpen && hasDetails ? (
          <div className="qzip-task-card__details">
            {task.error ? <span><strong>{text("错误代码", "Error code")}</strong>{task.error.code}</span> : null}
            {task.warnings.length ? <span><strong>{text("警告", "Warnings")}</strong>{task.warnings.join(text("；", "; "))}</span> : null}
          </div>
        ) : null}
        {needsPassword && showPassword ? <Input aria-label={text("重试密码", "Retry password")} type="password" value={password} onChange={(event) => setPassword(event.target.value)} placeholder={text("请输入正确密码", "Enter the correct password")} /> : null}
      </div>
      <div className="qzip-task-card__actions">
        {active ? <Button variant="icon" aria-label={text("暂停任务（暂不支持）", "Pause task (not supported)")} disabled title={text("当前版本暂不支持暂停任务", "Pausing tasks is not supported yet")} icon={<PauseRegular fontSize={22} />} /> : null}
        {active ? <Button variant="danger" icon={<DismissRegular fontSize={19} />} onClick={() => onCancel(task.taskId)}>{text("取消任务", "Cancel task")}</Button> : null}
        {task.status === "failed" && task.retryable ? (
          <Button
            variant="warning"
            icon={<WarningRegular fontSize={19} />}
            disabled={showPassword && needsPassword && !password}
            onClick={() => showPassword ? onRetry(task.taskId, password || undefined) : setShowPassword(true)}
          >
            {showPassword ? text("确认重试", "Retry") : needsPassword ? text("重新输入密码", "Enter password") : text("重试", "Retry")}
          </Button>
        ) : null}
        {hasDetails ? <Button variant="tertiary" icon={<MoreHorizontalRegular fontSize={19} />} onClick={() => setDetailsOpen((current) => !current)}>{detailsOpen ? text("收起详情", "Hide details") : text("查看详情", "View details")}</Button> : null}
        {task.status === "completed" && task.output ? <Button variant="primary" icon={<OpenRegular fontSize={19} />} onClick={() => void archiveClient.open(task.output!)}>{text("打开结果", "Open result")}</Button> : null}
        {task.output ? <Button variant="secondary" onClick={() => void archiveClient.reveal(task.output!)}>{text("打开位置", "Open location")}</Button> : null}
      </div>
    </Card>
  );
}
