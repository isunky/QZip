import {
  forwardRef,
  type HTMLAttributes
} from "react";
import { classNames } from "./classNames";

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  elevated?: boolean;
}

export const Card = forwardRef<HTMLDivElement, CardProps>(function Card(
  { className, elevated = false, ...props },
  ref
) {
  return (
    <div
      {...props}
      ref={ref}
      className={classNames(
        "qzip-card",
        elevated && "qzip-card--elevated",
        className
      )}
    />
  );
});
