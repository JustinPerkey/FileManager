import type { StepId } from "../../lib/generated/StepId";

export interface StepCopy {
  title: string;
  description: string;
  /** Shown in a confirmation before the step runs on its own or as part of Run all. `null` runs without asking. */
  confirm: string | null;
}

/** One entry per step. A `Record` over the closed union: a missing or extra step fails the typecheck. */
export const stepCopy: Record<StepId, StepCopy> = {
  Verify: {
    title: "Check required files",
    description:
      "Checks that every subfolder has the files it needs. Nothing is changed.",
    confirm: null,
  },
  RunBatch: {
    title: "Run batch files",
    description:
      "Runs the batch file in each subfolder and reports the ones that fail.",
    confirm: "The batch file in each subfolder will be run.",
  },
  Schedule: {
    title: "Schedule",
    description: "Runs the schedule step for each subfolder.",
    confirm: "The schedule step may update files in each subfolder.",
  },
  Tarpack: {
    title: "Pack archives",
    description: "Packs each subfolder into a tar archive.",
    confirm: "An archive will be written for each subfolder.",
  },
};
