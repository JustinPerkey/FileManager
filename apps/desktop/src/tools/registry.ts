import type { ComponentType } from "react";
import { TarpackView } from "./tarpack/TarpackView";

export interface ToolEntry {
  id: string;
  label: string;
  view: ComponentType;
}

export const tools: ToolEntry[] = [{ id: "tarpack", label: "Tar Packager", view: TarpackView }];
