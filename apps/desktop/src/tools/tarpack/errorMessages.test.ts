import { expect, test } from "vitest";
import type { TarpackErrorKind } from "../../lib/generated/TarpackErrorKind";
import { errorMessage, toTarpackError } from "./errorMessages";
import { entry } from "./fixtures";

const kinds: TarpackErrorKind[] = [
  "NoManifest", "ManifestUnreadable", "NoEntries", "ManifestChangedOnDisk", "UnknownEntry",
  "NotAFile", "NoOutput", "EntriesNotReady", "OutputExists", "PathExists", "SourceMissing",
  "SourceUnreadable", "SourceChanged", "VerifyFailed", "BuildInProgress", "OpenerFailed", "Io",
];

test.each(kinds)("%s has a message", (kind) => {
  expect(errorMessage({ kind, message: "x" }, []).length).toBeGreaterThan(0);
});

test("there are 17 kinds including NoEntries", () => {
  expect(kinds).toHaveLength(17);
  expect(kinds).toContain("NoEntries");
});

test("source is filled from the entry id", () => {
  const entries = [entry("gw", "gateway.bin")];
  expect(errorMessage({ kind: "SourceMissing", message: "x", entryId: "gw" }, entries)).toBe(
    "gateway.bin is no longer at its assigned location.",
  );
});

test("reads 'A file' when the entry id is omitted or unknown", () => {
  expect(errorMessage({ kind: "SourceMissing", message: "x" }, [entry("gw")])).toMatch(/^A file /);
  expect(errorMessage({ kind: "NotAFile", message: "x", entryId: "nope" }, [entry("gw")])).toBe(
    "A file: the chosen path is not a file.",
  );
});

test("toTarpackError passes a valid error through unchanged", () => {
  const e = { kind: "PathExists" as const, message: "x", entryId: "a" };
  expect(toTarpackError(e)).toBe(e);
  const bare = { kind: "Io" as const, message: "y" };
  expect(toTarpackError(bare)).toBe(bare);
});

test.each([
  ["unknown kind", { kind: "Nope", message: "m" }, "[object Object]"],
  ["inherited kind", { kind: "toString", message: "m" }, "[object Object]"],
  ["missing message", { kind: "Io" }, "[object Object]"],
  ["bad entryId", { kind: "Io", message: "m", entryId: 3 }, "[object Object]"],
  ["a string", "boom", "boom"],
  ["an Error", new Error("thrown"), "thrown"],
  ["undefined", undefined, "undefined"],
])("toTarpackError turns %s into Io", (_n, input, text) => {
  expect(toTarpackError(input)).toEqual({ kind: "Io", message: text });
});
