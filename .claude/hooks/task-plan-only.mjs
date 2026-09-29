#!/usr/bin/env node
// PreToolUse guard for the implementer and ui-implementer subagents.
//
// Implementers work from exactly one task plan in docs/plans/tasks/. Project
// plans (docs/plans/project/) and every other file under docs/plans/ outside
// tasks/ are off-limits to them. This blocks the obvious routes to those files:
// Read/Edit/Write on them, Grep/Glob whose search would cover them, and Bash
// commands that name them. It is a guardrail against accidental reads, not a
// sandbox — the agent instructions carry the rule; this makes slips loud.
//
// Exit 0 allows the call. Exit 2 blocks it and feeds stderr back to the agent.

import path from "node:path";

const PLANS = "docs/plans";
const ALLOWED = "docs/plans/tasks";
const FORBIDDEN_MARKER = "plans/project";

let input;
try {
  input = JSON.parse((await readStdin()).trim() || "{}");
} catch {
  process.exit(0); // unreadable hook input: a guardrail fails open
}
const tool = input.tool_name ?? "";
const ti = input.tool_input ?? {};
const root = normalize(process.env.CLAUDE_PROJECT_DIR || input.cwd || process.cwd());
const cwd = normalize(input.cwd || root);

const reason = check();
if (reason) {
  process.stderr.write(
    `Blocked: ${reason}\n` +
      "Implementers read only the task plan named in their prompt " +
      `(under ${ALLOWED}/). If that plan is missing something you need, stop ` +
      "and report the gap instead of looking for it in a project plan.\n",
  );
  process.exit(2);
}
process.exit(0);

function check() {
  switch (tool) {
    case "Read":
    case "Edit":
    case "Write":
    case "NotebookEdit": {
      const p = ti.file_path ?? ti.notebook_path;
      return p && isForbiddenFile(p) ? `${tool} of ${p}` : null;
    }
    case "Glob": {
      if (mentionsForbidden(ti.pattern) || mentionsForbidden(ti.path)) {
        return `Glob naming ${FORBIDDEN_MARKER}`;
      }
      return null;
    }
    case "Grep": {
      if (mentionsForbidden(ti.path) || mentionsForbidden(ti.glob)) {
        return `Grep naming ${FORBIDDEN_MARKER}`;
      }
      const searchRoot = rel(ti.path ?? cwd);
      if (searchRoot !== null && coversPlans(searchRoot) && !excludesMarkdown(ti)) {
        return (
          `Grep over "${searchRoot || "."}" would search ${PLANS}/. ` +
          "Scope it to a source directory (crates/, apps/, ...), or set " +
          'type (e.g. "rust", "ts") or a non-markdown glob'
        );
      }
      return null;
    }
    case "Bash":
      return mentionsForbidden(ti.command) ? `Bash command naming ${FORBIDDEN_MARKER}` : null;
    default:
      return null;
  }
}

// A file is forbidden when it sits under docs/plans/ but not under docs/plans/tasks/.
function isForbiddenFile(p) {
  const r = rel(p);
  if (r === null) return false;
  return within(r, PLANS) && !within(r, ALLOWED);
}

// True when a search rooted at r would descend into docs/plans/ beyond tasks/.
function coversPlans(r) {
  return r === "" || within(PLANS, r) || (within(r, PLANS) && !within(r, ALLOWED));
}

// True when a Grep's type or glob restricts it to non-markdown files.
function excludesMarkdown(t) {
  const isMd = (ext) => ["md", "markdown"].includes(ext.toLowerCase());
  if (t.type) return !isMd(String(t.type));
  if (t.glob) {
    const exts = [...String(t.glob).matchAll(/\.\{?([a-z0-9,]+)\}?/gi)].flatMap((m) =>
      m[1].split(","),
    );
    return exts.length > 0 && !exts.some(isMd);
  }
  return false;
}

function mentionsForbidden(s) {
  return typeof s === "string" && normalize(s).includes(FORBIDDEN_MARKER);
}

// Repo-relative, forward-slashed path; null when outside the repository.
function rel(p) {
  const abs = normalize(path.resolve(cwd, String(p)));
  const r = path.posix.relative(root, abs);
  if (r.startsWith("..") || path.posix.isAbsolute(r)) return null;
  return r;
}

function within(child, parent) {
  return child === parent || child.startsWith(parent + "/");
}

// Forward slashes, no trailing slash; lower-cased on Windows, whose paths are
// case-insensitive.
function normalize(p) {
  const s = String(p).replace(/\\/g, "/").replace(/\/+$/, "");
  return process.platform === "win32" ? s.toLowerCase() : s;
}

async function readStdin() {
  let data = "";
  for await (const chunk of process.stdin) data += chunk;
  return data || "{}";
}
