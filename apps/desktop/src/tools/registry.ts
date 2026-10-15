import type { ComponentType } from "react";
import { PipelineView } from "./pipeline/PipelineView";
import { ScheduleView } from "./schedule/ScheduleView";
import { TarpackView } from "./tarpack/TarpackView";

export interface ToolEntry {
  id: string;
  label: string;
  view: ComponentType;
}

export const tools: ToolEntry[] = [
  { id: "pipeline", label: "Pipeline", view: PipelineView },
  { id: "tarpack", label: "Tar Packager", view: TarpackView },
  { id: "schedule", label: "Schedule Creator", view: ScheduleView },
];
