import { pathParts } from "./pathParts";

/**
 * A display path on one line: CSS shortens the head first, so the drive and
 * the file name stay visible. The full path is the accessible text and the
 * `title`, and replaces the line while its row holds keyboard focus.
 */
export function MiddlePath({ path }: { path: string }) {
  const { head, tail } = pathParts(path);
  return (
    <span className="middle-path mono" title={path}>
      <span className="middle-path__line" aria-hidden="true">
        <span className="middle-path__head">{head}</span>
        <span className="middle-path__tail">{tail}</span>
      </span>
      <span className="middle-path__full">{path}</span>
    </span>
  );
}
