import {
  forwardRef,
  type ButtonHTMLAttributes,
  type ReactNode
} from "react";
import { classNames } from "./classNames";

export type ButtonVariant =
  | "primary"
  | "secondary"
  | "tertiary"
  | "danger"
  | "icon";

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  loading?: boolean;
  icon?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    {
      className,
      children,
      variant = "primary",
      loading = false,
      icon,
      disabled,
      type = "button",
      ...props
    },
    ref
  ) {
    const isDisabled = disabled || loading;

    return (
      <button
        {...props}
        ref={ref}
        type={type}
        className={classNames(
          "qzip-button",
          "qzip-button--" + variant,
          className
        )}
        disabled={isDisabled}
        aria-busy={loading || undefined}
      >
        {loading ? <span className="qzip-button__spinner" aria-hidden="true" /> : icon}
        {children ? <span className="qzip-button__label">{children}</span> : null}
      </button>
    );
  }
);
