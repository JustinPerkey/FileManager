const { chromium } = require('playwright');
const fs = require('fs');
const MOCK = fs.readFileSync(__dirname + '/mock.js', 'utf8');
const FONT = ':root{--font-ui:"Inter",system-ui,sans-serif !important;--font-mono:"DejaVu Sans Mono",monospace !important}';
let browser;
async function open(scenario, { width = scenario.wide ? 1366 : 1000, height = scenario.wide ? 800 : 700, colorScheme = 'light' } = {}) {
  browser ||= await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: 1, colorScheme });
  const page = await ctx.newPage();
  page.on('console', (m) => m.type() === 'warning' || m.type() === 'error' ? console.log('console:', m.text()) : null);
  page.on('pageerror', (e) => console.log('pageerror:', e.message));
  await page.addInitScript((s) => { window.__SCENARIO__ = s; }, scenario);
  await page.addInitScript(MOCK);
  await page.goto('http://localhost:1420/');
  await page.addStyleTag({ content: FONT });
  await page.waitForTimeout(400);
  return page;
}
async function tool(page, label) {
  await page.getByRole('button', { name: label, exact: true }).or(page.getByRole('link', { name: label })).or(page.getByRole('tab', { name: label })).first().click();
  await page.waitForTimeout(300);
}
async function snap(page, name, opts = {}) {
  await page.mouse.move(-1, -1).catch(() => {});
  await page.screenshot({ path: `${__dirname}/../images/${name}.png`, ...opts });
  console.log('shot', name);
}
async function close() { await browser?.close(); }
module.exports = { open, tool, snap, close };
