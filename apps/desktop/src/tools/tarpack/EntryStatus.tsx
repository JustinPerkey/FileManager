import { Icon, type IconName } from "../../app/icons";
import type { EntryStatus as Status } from "../../lib/generated/EntryStatus";

const spec: Record<Status, { label: string; icon: IconName }> = {
  ready: { label: "Ready", icon: "check-circle" },
  missing: { label: "Missing", icon: "alert-triangle" },
  unassigned: { label: "Not assigned", icon: "circle" },
};

export function EntryStatus({ status }: { status: Status }) {
  const { label, icon } = spec[status];
  return (
    <span className={`entry-status entry-status--${status}`}>
      <Icon name={icon} />
      <span>{label}</span>
    </span>
  );
}
