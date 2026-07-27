import { forwardRef, useId, type InputHTMLAttributes, type ReactNode } from "react";
import { classNames } from "./classNames";

export interface InputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  label?: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  trailing?: ReactNode;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { className, label, hint, error, trailing, id, ...props },
  ref
) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const descriptionId = hint || error ? inputId + "-description" : undefined;

  return (
    <div className="qzip-input-field">
      {label ? (
        <label className="qzip-input-field__label" htmlFor={inputId}>
          {label}
        </label>
      ) : null}
      <span
        className={classNames(
          "qzip-input-field__control",
          Boolean(error) && "is-error",
          props.disabled && "is-disabled"
        )}
      >
        <input
          {...props}
          ref={ref}
          id={inputId}
          className={classNames("qzip-input", className)}
          aria-describedby={descriptionId}
          aria-invalid={error ? true : undefined}
        />
        {trailing ? <span className="qzip-input-field__trailing">{trailing}</span> : null}
      </span>
      {error || hint ? (
        <span
          id={descriptionId}
          className={classNames(
            "qzip-input-field__description",
            Boolean(error) && "is-error"
          )}
        >
          {error || hint}
        </span>
      ) : null}
    </div>
  );
});
