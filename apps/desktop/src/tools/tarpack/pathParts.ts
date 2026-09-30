const NAME_CAP = 32;
const isSep = (c: string) => c === "\\" || c === "/";

/** Number of code points in the drive (`C:\`, `C:/`, `C:`) or UNC (`\\server\share\`) prefix. */
function prefixLength(cps: string[]): number {
  const s = cps.join("");
  const m = /^(?:[A-Za-z]:[\\/]?|\\\\[^\\/]+[\\/][^\\/]+[\\/]?)/.exec(s);
  // The prefix is ASCII except for server and share names, which may hold
  // astral characters, so count code points rather than UTF-16 units.
  return m ? Array.from(m[0]).length : 0;
}

/**
 * Split a display path into the part CSS may shorten (`head`, everything before
 * the last separator) and the part that stays visible (`tail`: the separator
 * and the file name). A file name over 32 code points keeps its last 32 after
 * an ellipsis. Works on code points, so it never splits a surrogate pair.
 */
export function pathParts(path: string): { head: string; tail: string } {
  const cps = Array.from(path);
  const pre = prefixLength(cps);
  const from = pre > 0 && isSep(cps[pre - 1]) ? pre - 1 : pre;
  let sep = -1;
  for (let i = cps.length - 1; i >= from; i--) {
    if (isSep(cps[i])) {
      sep = i;
      break;
    }
  }
  const head = sep < 0 ? "" : cps.slice(0, sep).join("");
  const lead = sep < 0 ? "" : cps[sep];
  const name = cps.slice(sep + 1);
  const shown = name.length > NAME_CAP ? `…${name.slice(-NAME_CAP).join("")}` : name.join("");
  return { head, tail: lead + shown };
}

/**
 * The folder of an assigned file's display path, for a dialog's start folder:
 * everything before the last separator, keeping the separator when only a
 * drive or UNC-share prefix remains. `undefined` when there is no separator or
 * the path is lossy (contains U+FFFD, so it must not be passed back to `lib`).
 * Works on code points.
 */
export function assignedFolder(path: string): string | undefined {
  if (path.includes("\uFFFD")) return undefined;
  const cps = Array.from(path);
  const pre = prefixLength(cps);
  const from = pre > 0 && isSep(cps[pre - 1]) ? pre - 1 : pre;
  let sep = -1;
  for (let i = cps.length - 1; i >= from; i--) {
    if (isSep(cps[i])) {
      sep = i;
      break;
    }
  }
  if (sep < 0) return undefined;
  return cps.slice(0, sep < Math.max(pre, 1) ? sep + 1 : sep).join("");
}
