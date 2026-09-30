import type { ReactNode } from "react";
import { Button } from "./Button";
import { Icon, type IconName } from "./icons";

type Tone = "info" | "warn" | "error";

const toneIcon: Record<Tone, IconName> = {
  info: "info",
  warn: "alert-triangle",
  error: "x-circle",
};
const toneWord: Record<Tone, string> = {
  info: "Information:",
  warn: "Warning:",
  error: "Error:",
};

interface BannerProps {
  tone: Tone;
  message: ReactNode;
  action?: { label: string; onAction: () => void };
  onDismiss?: () => void;
  children?: ReactNode;
}

export function Banner({ tone, message, action, onDismiss, children }: BannerProps) {
  return (
    <div className={`banner banner--${tone}`} role={tone === "error" ? "alert" : undefined}>
      <span className="banner__icon">
        <Icon name={toneIcon[tone]} />
      </span>
      <div className="banner__body">
        <span className="visually-hidden">{toneWord[tone]} </span>
        {message}
        {children}
      </div>
      <div className="banner__actions">
        {action && <Button onClick={action.onAction}>{action.label}</Button>}
        {onDismiss && (
          <Button variant="quiet" icon="x" aria-label="Dismiss" onClick={onDismiss} />
        )}
      </div>
    </div>
  );
}
