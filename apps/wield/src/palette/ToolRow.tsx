import type { ToolSummary } from "../lib/wield";
import { CategoryIcon } from "./CategoryIcon";
import "./ToolRow.css";

export interface ToolRowProps {
  tool: ToolSummary;
  selected: boolean;
  onSelect: () => void;
  onActivate: () => void;
}

export function ToolRow({ tool, selected, onSelect, onActivate }: ToolRowProps) {
  const activate = () => {
    onSelect();
    if (tool.available) {
      onActivate();
    }
  };

  return (
    <button
      className={`tool-row${selected ? " tool-row--selected" : ""}${tool.available ? "" : " tool-row--unavailable"}`}
      type="button"
      aria-pressed={selected}
      aria-disabled={!tool.available}
      onClick={activate}
      onFocus={onSelect}
      onMouseEnter={onSelect}
    >
      <span className="tool-row__icon" aria-hidden="true">
        <CategoryIcon category={tool.category} />
      </span>
      <span className="tool-row__title">{tool.title}</span>
      <span className="tool-row__meta">{tool.available ? tool.category : tool.reason}</span>
    </button>
  );
}
