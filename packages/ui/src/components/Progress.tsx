import type { CSSProperties } from "react";
import { classNames } from "./classNames";

export interface ProgressProps {
  value?: number;
  label?: string;
  className?: string;
}

export function Progress({ value, label, className }: ProgressProps) {
  const isIndeterminate = value === undefined;
  const clampedValue = Math.max(0, Math.min(100, value ?? 0));
  const style = {
    "--qzip-progress-value": clampedValue + "%"
  } as CSSProperties;

  return (
    <div
      className={classNames(
        "qzip-progress",
        isIndeterminate && "qzip-progress--indeterminate",
        className
      )}
      role="progressbar"
      aria-label={label}
      aria-valuemin={isIndeterminate ? undefined : 0}
      aria-valuemax={isIndeterminate ? undefined : 100}
      aria-valuenow={isIndeterminate ? undefined : clampedValue}
    >
      <span className="qzip-progress__bar" style={style} />
    </div>
  );
}
