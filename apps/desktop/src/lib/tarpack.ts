import type { ArchiveFormat } from "./generated/ArchiveFormat";
import type { BuildSummary } from "./generated/BuildSummary";
import type { DroppedAssignment } from "./generated/DroppedAssignment";
import type { ManifestChanged } from "./generated/ManifestChanged";
import type { Progress } from "./generated/Progress";
import type { TarpackSession } from "./generated/TarpackSession";
import { call, on } from "./tauri";
import type { UnlistenFn } from "@tauri-apps/api/event";

/** Typed client for the Tar Packager. Every mutating call resolves to a full session snapshot. */

export const MANIFEST_CHANGED_EVENT = "tarpack://manifest-changed";
export const BUILD_PROGRESS_EVENT = "tarpack://build-progress";

export const session = () => call<TarpackSession>("tarpack_session");
export const openManifest = (path: string) =>
  call<TarpackSession>("tarpack_open_manifest", { path });
export const reloadManifest = () => call<TarpackSession>("tarpack_reload_manifest");
export const assignDropped = (paths: string[]) =>
  call<DroppedAssignment>("tarpack_assign_dropped", { paths });
export const assign = (id: string, path: string) =>
  call<TarpackSession>("tarpack_assign", { id, path });
export const clear = (id: string) => call<TarpackSession>("tarpack_clear", { id });
export const setOutput = (path: string) => call<TarpackSession>("tarpack_set_output", { path });
export const setFormat = (format: ArchiveFormat) =>
  call<TarpackSession>("tarpack_set_format", { format });

/**
 * Builds the passed entries into the session's output. The returned summary is
 * the final report. Its `path` and `extractCommand` are for display and
 * copying only: never parse them back into paths. Use `revealOutput` to show
 * the file.
 */
export const build = (overwrite: boolean) => call<BuildSummary>("tarpack_build", { overwrite });

export const recentManifests = () => call<string[]>("tarpack_recent_manifests");
export const openInEditor = () => call<void>("tarpack_open_in_editor");

/** Reveals the file the last successful build in this session wrote. Takes no path. */
export const revealOutput = () => call<void>("tarpack_reveal_output");

export const createManifestFromExample = (path: string) =>
  call<TarpackSession>("tarpack_create_manifest_from_example", { path });

export const onManifestChanged = (handler: (payload: ManifestChanged) => void): Promise<UnlistenFn> =>
  on<ManifestChanged>(MANIFEST_CHANGED_EVENT, handler);

export const onBuildProgress = (handler: (payload: Progress) => void): Promise<UnlistenFn> =>
  on<Progress>(BUILD_PROGRESS_EVENT, handler);
