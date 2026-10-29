const { open, tool, snap, close } = require('./lib');
const only = process.argv[2];
const shots = {
  // ---- Tar Packager ----
  async 'tarpack-empty'() { const p = await open({ wide: 1 }); await tool(p, 'Tar Packager'); await snap(p, 'tarpack-empty'); },
  async 'tarpack-loaded'() { const p = await open({ wide: 1, tarpack: 'loaded' }); await tool(p, 'Tar Packager'); await snap(p, 'tarpack-loaded'); },
  async 'tarpack-recent'() { const p = await open({ wide: 1, tarpack: 'loaded' }); await tool(p, 'Tar Packager');
    await p.getByRole('button', { name: 'Recent' }).click(); await p.waitForTimeout(150); await snap(p, 'tarpack-recent'); },
  async 'tarpack-shortcuts'() { const p = await open({ wide: 1, tarpack: 'loaded' }); await tool(p, 'Tar Packager');
    await p.locator('.shortcuts__button').click(); await p.waitForTimeout(200); await snap(p, 'tarpack-shortcuts'); },
  async 'tarpack-drag'() { const p = await open({ wide: 1, tarpack: 'loaded' }); await tool(p, 'Tar Packager');
    await p.evaluate(() => window.__mock.emit('tauri://drag-enter', { paths: ['C:\\Builds\\gateway'], position: { x: 100, y: 100 } }));
    await p.waitForTimeout(200); await snap(p, 'tarpack-drag'); },
  async 'tarpack-dropped'() { const p = await open({ wide: 1, tarpack: 'loaded' }); await tool(p, 'Tar Packager');
    await p.evaluate(() => window.__mock.emit('tauri://drag-drop', { paths: [
      'C:\\Builds\\gateway\\release\\gateway', 'C:\\Builds\\gateway\\scripts\\start.sh', 'C:\\Builds\\gateway\\config\\gateway.conf',
      'C:\\Builds\\gateway\\README.md'], position: { x: 100, y: 100 } }));
    await p.waitForTimeout(400); await snap(p, 'tarpack-dropped'); },
  async 'tarpack-partial'() { const p = await open({ wide: 1, tarpack: 'partial' }); await tool(p, 'Tar Packager'); await snap(p, 'tarpack-partial'); },
  async 'tarpack-ready'() { const p = await open({ wide: 1, tarpack: 'ready' }); await tool(p, 'Tar Packager'); await snap(p, 'tarpack-ready'); },
  async 'tarpack-format'() { const p = await open({ wide: 1, tarpack: 'ready' }); await tool(p, 'Tar Packager');
    await snap(p, 'tarpack-buildbar', { clip: await p.locator('.build-bar').boundingBox() }); },
  async 'tarpack-building'() { const p = await open({ wide: 1, tarpack: 'ready', slowBuild: true }); await tool(p, 'Tar Packager');
    await p.getByRole('button', { name: 'Create archive' }).click(); await p.waitForTimeout(16000); await snap(p, 'tarpack-building'); },
  async 'tarpack-result'() { const p = await open({ wide: 1, tarpack: 'ready' }); await tool(p, 'Tar Packager');
    await p.getByRole('button', { name: 'Create archive' }).click(); await p.waitForTimeout(1500);
    await p.locator('.build-result').scrollIntoViewIfNeeded(); await snap(p, 'tarpack-result');
  },
  async 'tarpack-result-partial'() { const p = await open({ wide: 1, tarpack: 'partial' }); await tool(p, 'Tar Packager');
    await p.getByRole('button', { name: 'Create archive' }).click(); await p.waitForTimeout(1500);
    await snap(p, 'tarpack-result-partial', { clip: await p.locator('.build-result').boundingBox() }); },
  async 'tarpack-replace'() { const p = await open({ wide: 1, tarpack: 'ready', outputExists: true }); await tool(p, 'Tar Packager');
    await p.getByRole('button', { name: 'Create archive' }).click(); await p.waitForTimeout(400); await snap(p, 'tarpack-replace'); },
  async 'tarpack-errors'() { const p = await open({ wide: 1, tarpack: 'errors' }); await tool(p, 'Tar Packager'); await snap(p, 'tarpack-errors'); },
  async 'tarpack-withheld'() { const p = await open({ wide: 1, tarpack: 'withheld' }); await tool(p, 'Tar Packager'); await snap(p, 'tarpack-withheld'); },
  async 'tarpack-missing'() { const p = await open({ wide: 1, tarpack: 'missing' }); await tool(p, 'Tar Packager'); await snap(p, 'tarpack-missing'); },
  async 'tarpack-changed'() { const p = await open({ wide: 1, tarpack: 'ready' }); await tool(p, 'Tar Packager');
    await p.evaluate(() => window.__mock.emit('tarpack://manifest-changed', { path: 'C:\\Packages\\gateway\\manifest.toml' }));
    await p.waitForTimeout(200); await snap(p, 'tarpack-changed'); },
  async 'tarpack-dev'() { const p = await open({ wide: 1, tarpack: 'ready' }); await tool(p, 'Tar Packager');
    await p.keyboard.press('Control+Alt+Shift+KeyD'); await p.waitForTimeout(200);
    await snap(p, 'tarpack-dev', { clip: await p.locator('.manifest-header').boundingBox() }); },
  async 'tarpack-dev-result'() { const p = await open({ wide: 1, tarpack: 'ready' }); await tool(p, 'Tar Packager');
    await p.keyboard.press('Control+Alt+Shift+KeyD'); await p.getByRole('button', { name: 'Create archive' }).click(); await p.waitForTimeout(1500);
    await snap(p, 'tarpack-dev-result', { clip: await p.locator('.build-result').boundingBox() }); },
  async 'tarpack-format-open'() { const p = await open({ wide: 1, tarpack: 'loaded' }); await tool(p, 'Tar Packager');
    await snap(p, 'tarpack-statewarn'); },
  // ---- Schedule Creator ----
  async 'schedule-empty'() { const p = await open({}); await tool(p, 'Schedule Creator'); await snap(p, 'schedule-empty'); },
  async 'schedule-ready'() { const p = await open({ schedule: 'ready' }); await tool(p, 'Schedule Creator'); await snap(p, 'schedule-ready'); },
  async 'schedule-bad'() { const p = await open({ schedule: 'bad' }); await tool(p, 'Schedule Creator'); await snap(p, 'schedule-bad'); },
  async 'schedule-confirm'() { const p = await open({ schedule: 'ready' }); await tool(p, 'Schedule Creator');
    await p.getByRole('button', { name: 'Add to XML…' }).click(); await p.waitForTimeout(300); await snap(p, 'schedule-confirm');
    await p.getByRole('button', { name: 'Update' }).click(); await p.waitForTimeout(800); await snap(p, 'schedule-notimpl'); },
  // ---- Pipeline ----
  async 'pipeline-empty'() { const p = await open({}); await snap(p, 'pipeline-empty'); },
  async 'pipeline-chosen'() { const p = await open({ pipeline: 'chosen' }); await snap(p, 'pipeline-chosen'); },
  async 'pipeline-bad'() { const p = await open({ pipeline: 'bad' }); await snap(p, 'pipeline-bad'); },
  async 'pipeline-runall'() { const p = await open({ pipeline: 'chosen' });
    await p.getByRole('button', { name: 'Run all…' }).click(); await p.waitForTimeout(300); await snap(p, 'pipeline-runall'); },
  async 'pipeline-running'() { const p = await open({ pipeline: 'chosen', slowStep: true });
    await p.getByRole('button', { name: /Run next/ }).click(); await p.waitForTimeout(600); await snap(p, 'pipeline-running'); },
  async 'pipeline-ran'() { const p = await open({ pipeline: 'chosen' });
    await p.getByRole('button', { name: /Run next/ }).click(); await p.waitForTimeout(1000); await snap(p, 'pipeline-ran'); },
};
(async () => {
  for (const [name, fn] of Object.entries(shots)) {
    if (only && !name.startsWith(only)) continue;
    try { await fn(); } catch (e) { console.log('FAIL', name, e.message.split('\n')[0]); }
  }
  await close();
})();
