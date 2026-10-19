import type { PipelineSession } from "./generated/PipelineSession";
import type { StepId } from "./generated/StepId";
import { call } from "./tauri";

/** Typed client for the Pipeline. Every call resolves to a full session snapshot. */

export const session = () => call<PipelineSession>("pipeline_session");

/**
 * Sets the folder the steps run over, or clears it with `null`. A bad path is
 * kept, with its error. Every step's result is forgotten. Fails with
 * `StepInProgress` while a step runs.
 */
export const setDirectory = (path: string | null) =>
  call<PipelineSession>("pipeline_set_directory", { path });

/**
 * Runs one step and resolves when it has finished, with its result recorded
 * on the step (`report`, or `error` when it failed or is not implemented).
 * Rejects only when the step could not start. Pass the folder path the UI
 * showed; if the session holds another, nothing runs (`DirectoryChanged`).
 */
export const runStep = (step: StepId, expectedDirectory: string) =>
  call<PipelineSession>("pipeline_run_step", { step, expectedDirectory });
