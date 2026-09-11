import type { ValueKind } from "../../lib/wield";
import "./outcomes.css";

export interface ValueResultProps {
  kind: ValueKind;
  data: string;
  onCopy: () => void;
}

function colorLines(data: string): string[] {
  try {
    const parsed: unknown = JSON.parse(data);
    if (typeof parsed === "object" && parsed !== null) {
      const color = parsed as Record<string, unknown>;
      if (
        typeof color.hex === "string" &&
        typeof color.rgb === "string" &&
        typeof color.hsl === "string"
      ) {
        return [color.hex, color.rgb, color.hsl];
      }
    }
  } catch {
    // Fall through to the raw payload so malformed data is still useful.
  }
  return [data];
}

export function ValueResult({ kind, data, onCopy }: ValueResultProps) {
  const lines = kind === "Color" ? colorLines(data) : [data];
  return (
    <div className="outcome-card outcome-value">
      <div className="outcome-label">Result</div>
      <div className="outcome-data outcome-value__lines">
        {lines.map((line) => (
          <div key={line}>{line}</div>
        ))}
      </div>
      <button type="button" className="outcome-action" onClick={onCopy}>
        Copy value
      </button>
    </div>
  );
}
