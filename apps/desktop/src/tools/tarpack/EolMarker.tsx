export function EolMarker() {
  return (
    <span
      className="eol-marker"
      title="Windows line endings (CRLF) are converted to Linux (LF) when the archive is written"
    >
      <span aria-hidden="true">CRLF → LF</span>
      <span className="visually-hidden">line endings converted to LF</span>
    </span>
  );
}
