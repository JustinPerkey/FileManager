import type { ApplySummary } from "./generated/ApplySummary";
import type { ScheduleSession } from "./generated/ScheduleSession";
import { call } from "./tauri";

/** Typed client for the Schedule Creator. Every call but `apply` resolves to a full session snapshot. */

export const session = () => call<ScheduleSession>("schedule_session");
export const openText = (path: string) => call<ScheduleSession>("schedule_open_text", { path });
export const reloadText = () => call<ScheduleSession>("schedule_reload_text");
export const openXml = (path: string) => call<ScheduleSession>("schedule_open_xml", { path });

/**
 * Adds the previewed schedule to the chosen XML file, after saving a backup
 * of it. Confirm with the user first: this rewrites the file.
 */
export const apply = () => call<ApplySummary>("schedule_apply");
