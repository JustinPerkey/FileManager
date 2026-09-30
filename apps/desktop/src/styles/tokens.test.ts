import css from "./tokens.css?raw";
import tarpackCss from "./tarpack.css?raw";
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

test.each(themes)("%s defines the overlay shadow", (_name, t) => {
  expect(t["--shadow-overlay"]).toMatch(/^0 8px 24px rgb\(0 0 0 \/ 0\.(18|5)\)$/);
});

test.each(themes)("%s defines --scrim", (name, t) => {
  expect(t["--scrim"]).toBe(name === "light" ? "rgb(0 0 0 / 0.4)" : "rgb(0 0 0 / 0.6)");
});

test("type-size tokens exist on :root", () => {
  const t = themes[0][1];
  expect(t["--font-size-sm"]).toBe("0.8125rem");
  expect(t["--font-size-md"]).toBe("0.875rem");
  expect(t["--font-size-lg"]).toBe("1rem");
  expect(t["--font-size-xl"]).toBe("1.25rem");
});

/** Split a selector list on commas that are not inside parentheses. */
function topLevelSplit(list: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let cur = "";
  for (const ch of list) {
    if (ch === "(") depth++;
    if (ch === ")") depth--;
    if (ch === "," && depth === 0) {
      out.push(cur);
      cur = "";
    } else cur += ch;
  }
  out.push(cur);
  return out;
}

test("every tarpack.css selector is scoped under .tarpack, and only the overlays have a shadow", () => {
  const rules = [...tarpackCss.replace(/\/\*[\s\S]*?\*\//g, "").replace(/@container[^{]*\{/g, "").matchAll(/([^{}]+)\{([^{}]*)\}/g)];
  expect(rules.length).toBeGreaterThan(0);
  const shadowed: string[] = [];
  for (const [, sel, body] of rules) {
    for (const one of topLevelSplit(sel)) expect(one.trim().startsWith(".tarpack")).toBe(true);
    if (/box-shadow/.test(body)) shadowed.push(sel.trim());
  }
  expect(shadowed).toEqual([".tarpack .shortcuts__popover", ".tarpack .menu__list"]);
});
