// Mock Tauri IPC for screenshots. Scenario in window.__SCENARIO__ (set by an earlier init script).
(() => {
  const S = window.__SCENARIO__ || {};
  const callbacks = new Map();
  const listeners = {}; // event -> [callbackId]
  let nextId = 1;
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const clone = (o) => JSON.parse(JSON.stringify(o));
  const base = (p) => p.split(/[\\/]/).pop();

  function emit(event, payload) {
    for (const id of listeners[event] || []) {
      const cb = callbacks.get(id);
      if (cb) cb({ event, id, payload });
    }
  }

  // ---------- Tar Packager ----------
  const FORMATS = [
    { format: "tar", extension: ".tar", filterExtension: "tar" },
    { format: "tarGz", extension: ".tar.gz", filterExtension: "gz" },
    { format: "tarZst", extension: ".tar.zst", filterExtension: "zst" },
    { format: "tarXz", extension: ".tar.xz", filterExtension: "xz" },
  ];
  const EXT = { tar: ".tar", tarGz: ".tar.gz", tarZst: ".tar.zst", tarXz: ".tar.xz" };
  const ent = (id, source, targetPath, mode, modeText, owner, extra = {}) => ({
    assigned: null, status: "unassigned", id, source, targetPath, mode, modeText, owner,
    uid: 0, gid: owner.endsWith("gateway") ? 990 : 0, normalizeEol: false, ...extra,
  });
  const GATEWAY_ENTRIES = [
    ent("gateway-bin", "gateway", "/opt/gateway/bin/gateway", "0755", "rwxr-xr-x", "root:root"),
    ent("gateway-start", "start.sh", "/opt/gateway/bin/start.sh", "0755", "rwxr-xr-x", "root:root", { normalizeEol: true }),
    ent("gateway-conf", "gateway.conf", "/etc/gateway/gateway.conf", "0640", "rw-r-----", "root:gateway"),
    ent("gateway-ca", "ca.pem", "/etc/gateway/tls/ca.pem", "0644", "rw-r--r--", "root:root"),
    ent("gateway-unit", "gateway.service", "/etc/systemd/system/gateway.service", "0644", "rw-r--r--", "root:root"),
  ];
  const MANIFEST_PATH = "C:\\Packages\\gateway\\manifest.toml";
  const gateway = () => ({
    path: MANIFEST_PATH, name: "Gateway deploy", outputName: "gateway.tar.zst", hash: "9f2c",
    entries: clone(GATEWAY_ENTRIES), entriesWithheld: false, errors: [], failedEntries: [],
    errorCount: 0, warnings: [],
  });
  const withErrors = () => {
    const m = gateway();
    m.entries = m.entries.filter((e) => e.id !== "gateway-ca");
    m.failedEntries = [{
      index: 4, id: "gateway-ca", source: "ca.pem", line: 33,
      errors: [{ severity: "error", line: 35, col: 7, entryId: "gateway-ca", message: "file `gateway-ca`: dir must be absolute (start with `/`)" },
               { severity: "error", line: 36, col: 8, entryId: "gateway-ca", message: "file `gateway-ca`: mode: mode `0898` is not an octal string" }],
    }];
    m.warnings = [{ severity: "warning", line: 41, col: 10, entryId: "gateway-unit", message: "file `gateway-unit`: source `gateway.service` matches file `gateway-svc` (compared case-insensitively); a dropped file cannot tell them apart, so pick their files per row" }];
    m.errorCount = 2;
    return m;
  };
  const ASSIGN = {
    "gateway-bin": "C:\\Builds\\gateway\\release\\gateway",
    "gateway-start": "C:\\Builds\\gateway\\scripts\\start.sh",
    "gateway-conf": "C:\\Builds\\gateway\\config\\gateway.conf",
    "gateway-ca": "C:\\Builds\\gateway\\tls\\ca.pem",
    "gateway-unit": "C:\\Builds\\gateway\\systemd\\gateway.service",
  };

  const tp = {
    manifest: null, outputPath: null, format: "tarZst", stateWarning: null,
    built: false,
  };
  if (S.tarpack === "loaded" || S.tarpack === "ready" || S.tarpack === "partial" || S.tarpack === "missing") tp.manifest = gateway();
  if (S.tarpack === "errors") tp.manifest = withErrors();
  if (S.tarpack === "withheld") {
    tp.manifest = gateway();
    tp.manifest.entries = [];
    tp.manifest.entriesWithheld = true;
    tp.manifest.errors = [{ severity: "error", line: 9, col: 1, entryId: null, message: "unknown key `default`" }];
    tp.manifest.errorCount = 1;
  }
  if (S.tarpack === "ready" || S.tarpack === "partial" || S.tarpack === "missing" || S.tarpack === "errors") {
    for (const e of tp.manifest.entries) {
      if (S.tarpack === "partial" && (e.id === "gateway-ca" || e.id === "gateway-unit")) continue;
      e.assigned = ASSIGN[e.id];
      e.status = "ready";
    }
    if (S.tarpack === "missing") tp.manifest.entries[2].status = "missing";
    tp.outputPath = "C:\\Releases\\gateway.tar.zst";
  }
  if (S.stateWarning) tp.stateWarning = S.stateWarning;

  function tsession() {
    const m = tp.manifest;
    const entries = m ? m.entries : [];
    const readyCount = entries.filter((e) => e.status === "ready").length;
    let reason = null;
    if (!m) reason = "noManifest";
    else if (entries.length === 0) reason = "noEntries";
    else if (entries.some((e) => e.status === "missing")) reason = "entriesNotReady";
    else if (readyCount === 0) reason = "nothingLoaded";
    else if (!tp.outputPath) reason = "noOutput";
    return {
      manifest: m ? clone(m) : null, outputPath: tp.outputPath, format: tp.format, formats: FORMATS,
      suggestedOutputName: m ? "gateway.tar.zst" : null, readyCount, totalCount: entries.length,
      canBuild: reason === null, buildBlockedReason: reason, stateWarning: tp.stateWarning,
    };
  }

  async function tarpackBuild(overwrite) {
    if (S.outputExists && !overwrite) throw { kind: "OutputExists", message: "C:\\Releases\\gateway.tar.zst exists" };
    const ids = tp.manifest.entries.filter((e) => e.status === "ready").map((e) => e.id);
    const total = 24_117_248;
    const steps = S.slowBuild ? 400 : 20;
    for (const phase of ["writing", "verifying"]) {
      for (let i = 1; i <= steps; i++) {
        const done = Math.round((total * i) / steps);
        emit("tarpack://build-progress", { phase, entryId: ids[Math.min(ids.length - 1, Math.floor((i / steps) * ids.length))], bytesDone: done, bytesTotal: total });
        await sleep(S.slowBuild ? 100 : 15);
      }
    }
    const m = tp.manifest;
    return {
      path: tp.outputPath, format: tp.format, entries: ids.length + 6, files: ids.length, dirs: 6,
      bytes: 6_291_874, uncompressedBytes: total,
      sha256Hex: "3b7e1f0c9a4d2e86b51c7f30a9e2d4c8f6a1b0e93d5c7f28a4e6b1d9c3f05a7e",
      extractCommand: `tar --zstd --no-overwrite-dir -xpPf ${base(tp.outputPath)}`,
      normalizedEntries: m.entries.filter((e) => e.normalizeEol && e.status === "ready").map((e) => ({ id: e.id, crlfReplaced: 42 })),
      builtIds: ids,
      leftOut: m.failedEntries,
      notLoaded: m.entries.filter((e) => e.status === "unassigned").map((e) => e.id),
      manifestErrors: m.errors, warnings: m.warnings, errorCount: m.errorCount,
    };
  }

  function dropped(paths) {
    const matched = [], unmatched = [];
    for (const p of paths) {
      const e = tp.manifest.entries.find((x) => x.source.toLowerCase() === base(p).toLowerCase());
      if (!e) unmatched.push({ path: p, reason: "noEntry" });
      else if (e.assigned === p) unmatched.push({ path: p, reason: "alreadyAssigned" });
      else { e.assigned = p; e.status = "ready"; matched.push([e.id, p]); }
    }
    return { session: tsession(), outcome: { matched, unmatched, ambiguous: [] } };
  }

  // ---------- Schedule Creator ----------
  const sc = { text: null, xml: null };
  if (S.schedule === "ready" || S.schedule === "bad") {
    sc.text = { path: "C:\\Schedules\\week-44.txt", error: null };
    sc.xml = { path: "C:\\Config\\schedule.xml", error: null };
  }
  if (S.schedule === "bad") sc.text = { path: "schedules\\week-44.txt", error: { kind: "NotAbsolute", message: "path is relative" } };
  const scSession = () => ({ text: sc.text, xml: sc.xml, canApply: !!(sc.text && sc.xml && !sc.text.error && !sc.xml.error) });
  const slot = (path) => {
    if (path === null) return null;
    if (!/^[A-Za-z]:\\/.test(path)) return { path, error: { kind: "NotAbsolute", message: "path is relative" } };
    return { path, error: null };
  };

  // ---------- Pipeline ----------
  const STEPS = ["Verify", "RunBatch", "Schedule", "Tarpack"];
  const pl = { directory: null, steps: STEPS.map((id, i) => ({ id, report: null, error: null, unlocked: i === 0 })), running: null };
  if (S.pipeline === "chosen" || S.pipeline === "ran") pl.directory = { path: "C:\\Deliveries\\2026-10", error: null, subdirectories: 12 };
  if (S.pipeline === "bad") pl.directory = { path: "C:\\Deliveries\\2026-13", error: { kind: "NotADirectory", message: "not found" }, subdirectories: 0 };
  if (S.pipeline === "ran") pl.steps[0].error = { kind: "StepNotImplemented", message: "verify is not implemented" };
  if (S.pipeline === "demo") {
    pl.directory = { path: "C:\\Deliveries\\2026-10", error: null, subdirectories: 12 };
    pl.steps[0].report = { summary: "12 folders checked", issues: [], passed: true };
    pl.steps[1].unlocked = true;
    pl.steps[1].report = { summary: "12 batch files run, 2 failed", passed: false, issues: [
      { subdirectory: "site-07", message: "build.bat exited with code 1" },
      { subdirectory: "site-11", message: "build.bat was not found" } ] };
  }
  const plSession = () => {
    const next = pl.steps.find((s) => !(s.report && s.report.passed));
    return { directory: pl.directory, steps: clone(pl.steps), nextStep: next ? next.id : null, running: pl.running,
      canRun: !!(pl.directory && !pl.directory.error && !pl.running) };
  };

  // ---------- dispatcher ----------
  const handlers = {
    "plugin:event|listen": ({ event, handler }) => { (listeners[event] ||= []).push(handler); return handler; },
    "plugin:event|unlisten": () => null,
    "plugin:dialog|open": () => { const q = window.__dialog || []; return q.length ? q.shift() : null; },
    "plugin:dialog|save": () => { const q = window.__dialog || []; return q.length ? q.shift() : null; },

    tarpack_session: () => tsession(),
    tarpack_recent_manifests: () => (tp.manifest || S.recent ? [MANIFEST_PATH, "C:\\Packages\\sensor-agent\\manifest.toml", "D:\\work\\edge-router\\deploy.toml"] : []),
    tarpack_open_manifest: () => { tp.manifest = gateway(); return tsession(); },
    tarpack_create_manifest_from_example: () => { tp.manifest = gateway(); return tsession(); },
    tarpack_reload_manifest: () => tsession(),
    tarpack_assign: ({ id, path }) => { const e = tp.manifest.entries.find((x) => x.id === id); e.assigned = path; e.status = "ready"; return tsession(); },
    tarpack_clear: ({ id }) => { const e = tp.manifest.entries.find((x) => x.id === id); e.assigned = null; e.status = "unassigned"; return tsession(); },
    tarpack_assign_dropped: ({ paths }) => dropped(paths),
    tarpack_set_output: ({ path }) => {
      tp.outputPath = path;
      for (const [f, ext] of Object.entries(EXT)) if (path.toLowerCase().endsWith(ext) && ext !== ".tar") tp.format = f;
      return tsession();
    },
    tarpack_set_format: ({ format }) => {
      tp.format = format;
      if (tp.outputPath) tp.outputPath = tp.outputPath.replace(/\.tar(\.(gz|zst|xz))?$/i, "") + EXT[format];
      return tsession();
    },
    tarpack_build: ({ overwrite }) => tarpackBuild(overwrite),
    tarpack_open_in_editor: () => null,
    tarpack_reveal_output: () => null,

    schedule_session: () => scSession(),
    schedule_set_text: ({ path }) => { sc.text = slot(path); return scSession(); },
    schedule_set_xml: ({ path }) => { sc.xml = slot(path); return scSession(); },
    schedule_set_dropped: ({ paths }) => { for (const p of paths) { if (p.toLowerCase().endsWith(".xml")) sc.xml = slot(p); else sc.text = slot(p); } return scSession(); },
    schedule_apply: async () => { await sleep(300); throw { kind: "ParseNotImplemented", message: "fm_schedule::parse is not implemented" }; },

    pipeline_session: () => plSession(),
    pipeline_set_directory: ({ path }) => {
      if (path === null) pl.directory = null;
      else pl.directory = /^[A-Za-z]:\\/.test(path) ? { path, error: null, subdirectories: 12 } : { path, error: { kind: "NotAbsolute", message: "relative" }, subdirectories: 0 };
      pl.steps.forEach((s, i) => { s.report = null; s.error = null; s.unlocked = i === 0; });
      return plSession();
    },
    pipeline_run_step: async ({ step }) => {
      pl.running = step;
      await sleep(S.slowStep ? 30000 : 400);
      pl.running = null;
      const s = pl.steps.find((x) => x.id === step);
      s.error = { kind: "StepNotImplemented", message: `${step} is not implemented` };
      return plSession();
    },
  };

  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback(cb, once) { const id = nextId++; callbacks.set(id, (p) => { if (once) callbacks.delete(id); cb(p); }); return id; },
    unregisterCallback(id) { callbacks.delete(id); },
    convertFileSrc: (p) => p,
    async invoke(cmd, args) {
      const h = handlers[cmd];
      if (!h) { console.warn("mock: unhandled", cmd, args); throw `unhandled ${cmd}`; }
      return h(args || {});
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.__mock = { emit };
})();
