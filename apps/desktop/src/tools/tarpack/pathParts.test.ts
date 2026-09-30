import { expect, test } from "vitest";
import { pathParts } from "./pathParts";

test("splits at the last separator", () => {
  expect(pathParts("C:\\Users\\me\\build\\out\\gateway.exe")).toEqual({
    head: "C:\\Users\\me\\build\\out",
    tail: "\\gateway.exe",
  });
});

test("drive-root file", () => {
  expect(pathParts("C:\\a.txt")).toEqual({ head: "C:", tail: "\\a.txt" });
  expect(pathParts("C:/x/a.txt")).toEqual({ head: "C:/x", tail: "/a.txt" });
});

test("UNC prefix stays in head", () => {
  expect(pathParts("\\\\srv\\share\\d\\f.bin")).toEqual({ head: "\\\\srv\\share\\d", tail: "\\f.bin" });
  expect(pathParts("\\\\srv\\share\\f.bin")).toEqual({ head: "\\\\srv\\share", tail: "\\f.bin" });
});

test("UNC server and share names may hold astral or non-ASCII characters", () => {
  expect(pathParts("\\\\s😀\\share\\d\\f.bin")).toEqual({ head: "\\\\s😀\\share\\d", tail: "\\f.bin" });
  expect(pathParts("\\\\sé\\sh😀re\\f.bin")).toEqual({ head: "\\\\sé\\sh😀re", tail: "\\f.bin" });
});

test("no separator: head empty, tail whole", () => {
  expect(pathParts("gateway.exe")).toEqual({ head: "", tail: "gateway.exe" });
});

test("200-character path keeps drive in head and name in tail", () => {
  const p = `C:\\${"d".repeat(190)}\\file.txt`;
  const { head, tail } = pathParts(p);
  expect(head.startsWith("C:\\")).toBe(true);
  expect(tail).toBe("\\file.txt");
  expect(head + tail).toBe(p);
});

test("name of exactly 32 code points is kept, 33 is capped", () => {
  const n32 = "a".repeat(32);
  expect(pathParts(`C:\\d\\${n32}`).tail).toBe(`\\${n32}`);
  const n33 = `Z${n32}`;
  expect(pathParts(`C:\\d\\${n33}`).tail).toBe(`\\…${n32}`);
});

test("an astral character at the cap is not split", () => {
  const name = `${"😀".repeat(40)}`;
  const { tail } = pathParts(`C:\\d\\${name}`);
  expect(Array.from(tail)).toEqual(["\\", "…", ...Array.from("😀".repeat(32))]);
});

test("U+FFFD is preserved", () => {
  const p = "C:\\d\uFFFD\\bad\uFFFD.txt";
  expect(pathParts(p)).toEqual({ head: "C:\\d\uFFFD", tail: "\\bad\uFFFD.txt" });
});
