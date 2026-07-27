import type { KeyboardEvent } from "react";
import { classNames } from "./classNames";

export interface SegmentedControlOption<T extends string> {
  value: T;
  label: string;
  disabled?: boolean;
}

export interface SegmentedControlProps<T extends string> {
  options: readonly SegmentedControlOption<T>[];
  value: T;
  onValueChange: (value: T) => void;
  ariaLabel: string;
  className?: string;
}

export function SegmentedControl<T extends string>({
  options,
  value,
  onValueChange,
  ariaLabel,
  className
}: SegmentedControlProps<T>) {
  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
      return;
    }

    event.preventDefault();
    const enabledOptions = options.filter((option) => !option.disabled);
    const currentIndex = enabledOptions.findIndex((option) => option.value === value);
    const nextIndex =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? enabledOptions.length - 1
          : (currentIndex +
              (event.key === "ArrowRight" ? 1 : -1) +
              enabledOptions.length) %
            enabledOptions.length;
    const nextOption = enabledOptions[nextIndex];

    if (nextOption) {
      onValueChange(nextOption.value);
      const parent = event.currentTarget.parentElement;
      const nextButton = parent?.querySelector<HTMLButtonElement>(
        '[data-segment-value="' + nextOption.value + '"]'
      );
      nextButton?.focus();
    }
  }

  return (
    <div
      className={classNames("qzip-segmented-control", className)}
      role="radiogroup"
      aria-label={ariaLabel}
    >
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          data-segment-value={option.value}
          className="qzip-segmented-control__option"
          aria-checked={option.value === value}
          disabled={option.disabled}
          tabIndex={option.value === value ? 0 : -1}
          onClick={() => onValueChange(option.value)}
          onKeyDown={onKeyDown}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
