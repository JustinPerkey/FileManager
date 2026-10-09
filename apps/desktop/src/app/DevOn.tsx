import type { ReactNode } from "react";
import { DevModeContext } from "./devMode";

/** Test wrapper: renders its children with developer mode on. */
export function DevOn({ children }: { children: ReactNode }) {
  return <DevModeContext.Provider value={true}>{children}</DevModeContext.Provider>;
}
