import { forwardRef, type ButtonHTMLAttributes } from "react";
import { Icon, type IconName } from "./icons";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "quiet";
  icon?: IconName;
  /** Place the icon after the label. */
  iconAfter?: boolean;
  /** Rotate the icon 180 degrees. */
  iconFlip?: boolean;
  busy?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", icon, iconAfter, iconFlip, busy, className, children, disabled, ...rest },
  ref,
) {
  const cls = ["btn", variant !== "secondary" ? `btn--${variant}` : "", className ?? ""]
    .filter(Boolean)
    .join(" ");
  const iconEl = icon ? <Icon name={icon} flip={iconFlip} /> : null;
  return (
    <button
      type="button"
      {...rest}
      ref={ref}
      className={cls}
      disabled={disabled || busy}
      aria-busy={busy ? true : undefined}
    >
      {!iconAfter && iconEl}
      {children}
      {iconAfter && iconEl}
    </button>
  );
});
