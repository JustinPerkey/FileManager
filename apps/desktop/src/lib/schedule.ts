import type { ApplySummary } from "./generated/ApplySummary";
import type { ScheduleSession } from "./generated/ScheduleSession";
import { call } from "./tauri";

/** Typed client for the Schedule Creator. Every call but `apply` resolves to a full session snapshot. */

export const session = () => call<ScheduleSession>("schedule_session");

/** Sets the schedule text file, or clears it with `null`. A bad path is kept, with its error. */
export const setText = (path: string | null) => call<ScheduleSession>("schedule_set_text", { path });

/** Sets the XML file, or clears it with `null`. A bad path is kept, with its error. */
export const setXml = (path: string | null) => call<ScheduleSession>("schedule_set_xml", { path });

/** Sets the files from a drop: a `.xml` file is the XML file, any other the schedule file. */
export const setDropped = (paths: string[]) => call<ScheduleSession>("schedule_set_dropped", { paths });

/**
 * Parses the schedule file and adds it to the XML file, after saving a backup
 * of it. Confirm with the user first: this rewrites the file. Pass the two
 * display paths the confirmation showed; if the session holds other files,
 * nothing is written and the call fails with `FilesChanged`.
 */
export const apply = (expectedText: string, expectedXml: string) =>
  call<ApplySummary>("schedule_apply", { expectedText, expectedXml });
