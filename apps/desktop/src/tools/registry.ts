import type { ComponentType } from "react";

export interface ToolEntry {
  id: string;
  label: string;
  view: ComponentType;
}

export const tools: ToolEntry[] = [];
