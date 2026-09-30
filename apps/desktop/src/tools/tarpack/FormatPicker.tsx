import { useId } from "react";
import type { ArchiveFormat } from "../../lib/generated/ArchiveFormat";
import type { ArchiveFormatOption } from "../../lib/generated/ArchiveFormatOption";

/** Short display name per format. The extension always comes from the session. */
export const FORMAT_NAME: Record<ArchiveFormat, string> = {
  tar: "tar",
  tarGz: "gzip",
  tarZst: "zstd",
  tarXz: "xz",
};

interface FormatPickerProps {
  formats: ArchiveFormatOption[];
  value: ArchiveFormat;
  disabled: boolean;
  onChange: (format: ArchiveFormat) => void;
}

export function FormatPicker({ formats, value, disabled, onChange }: FormatPickerProps) {
  const id = useId();
  return (
    <div className="format-picker">
      <label htmlFor={id} className="format-picker__label">
        Format
      </label>
      <select
        id={id}
        className="format-picker__select"
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value as ArchiveFormat)}
      >
        {formats.map((f) => (
          <option key={f.format} value={f.format}>
            {FORMAT_NAME[f.format]} ({f.extension})
          </option>
        ))}
      </select>
    </div>
  );
}
