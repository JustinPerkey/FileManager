import css from "./tokens.css?raw";
import { expect, test } from "vitest";

function block(selectorStart: string): Record<string, string> {
  const start = css.indexOf(selectorStart);
  if (start < 0) throw new Error(`missing ${selectorStart}`);
  const open = css.indexOf("{", start);
  const close = css.indexOf("}", open);
  const out: Record<string, string> = {};
  for (const m of css.slice(open + 1, close).matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
    out[m[1]] = m[2].trim();
  }
  return out;
}

function lum(hex: string): number {
  expect(hex).toMatch(/^#[0-9a-f]{6}$/i);
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [n >> 16, (n >> 8) & 255, n & 255].map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const themes: [string, Record<string, string>][] = [
  ["light", block(":root {")],
  ["dark (media)", block(':root:not([data-theme="light"])')],
  ["dark (attribute)", block(':root[data-theme="dark"]')],
];

test("dark hooks define identical values", () => {
  expect(themes[1][1]).toEqual(themes[2][1]);
});

for (const [name, t] of themes.filter(([n]) => n !== "dark (attribute)")) {
  const pairs: [string, string][] = [];
  for (const fg of ["--text", "--text-muted"])
    for (const bg of ["--bg", "--surface", "--surface-sunken"]) pairs.push([fg, bg]);
  pairs.push(["--accent-text", "--accent"]);
  for (const fg of ["--ok", "--warn", "--danger"]) pairs.push([fg, "--surface"]);

  test.each(pairs)(`${name}: %s on %s meets 4.5:1`, (fg, bg) => {
    expect(contrast(t[fg], t[bg])).toBeGreaterThanOrEqual(4.5);
  });
}
