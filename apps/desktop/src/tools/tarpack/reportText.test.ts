import { expect, test } from "vitest";
import { diag, failure, summary } from "./fixtures";
import { reportText, resultAnnouncement, resultHeading } from "./reportText";

test("full report: heading, left out, manifest errors, warnings", () => {
  const s = summary({
    leftOut: [
      failure(1, { id: "a", source: "a.bin", line: 5, errors: [diag("bad dir", 6, 3), diag("bad mode", 7, 1)] }),
      failure(2, { id: null, source: null, line: 9, errors: [diag("no id", 10, 1)] }),
    ],
    manifestErrors: [diag("bad name", 1, 1)],
    warnings: [{ ...diag("odd mode", 4, 2), severity: "warning" }],
    errorCount: 4,
  });
  expect(reportText(s)).toBe(
    [
      "Created gateway.tar.zst with 2 files left out",
      "",
      "Left out of the archive:",
      "a (source a.bin, line 5)",
      "  6:3 bad dir",
      "  7:1 bad mode",
      "Entry #2 (line 9)",
      "  10:1 no id",
      "",
      "Manifest errors:",
      "  1:1 bad name",
      "",
      "Warnings:",
      "  4:2 odd mode",
    ].join("\n"),
  );
});

test("empty sections are omitted", () => {
  const s = summary({ manifestErrors: [diag("bad name", 1, 1)], errorCount: 1 });
  expect(reportText(s)).toBe("Created gateway.tar.zst with 1 manifest error\n\nManifest errors:\n  1:1 bad name");
});

test("announcements", () => {
  expect(resultAnnouncement(summary())).toBe("Created gateway.tar.zst.");
  expect(resultHeading(summary({ leftOut: [failure(1)], errorCount: 1 }))).toBe(
    "Created gateway.tar.zst with 1 file left out",
  );
  expect(resultAnnouncement(summary({ leftOut: [failure(1)], errorCount: 1 }))).toBe(
    "Created gateway.tar.zst. 1 file was left out because of errors.",
  );
  expect(resultAnnouncement(summary({ leftOut: [failure(1), failure(2)], errorCount: 2 }))).toBe(
    "Created gateway.tar.zst. 2 files were left out because of errors.",
  );
  expect(resultAnnouncement(summary({ manifestErrors: [diag("x"), diag("y")], errorCount: 2 }))).toBe(
    "Created gateway.tar.zst with 2 manifest errors.",
  );
});

test("a partial build names the files with no source chosen", () => {
  const s = summary({ notLoaded: ["gateway"], builtIds: ["core"] });
  expect(resultHeading(s)).toBe("Created gateway.tar.zst with 1 file not included");
  expect(reportText(s)).toContain("Not included (no file chosen):\n  gateway");
});
