import type { ToolSummary } from "../lib/wield";
import { ToolRow } from "./ToolRow";
import "./SearchView.css";

export interface SearchViewProps {
  query: string;
  onQueryChange: (query: string) => void;
  tools: ToolSummary[];
  showingRecents: boolean;
  selectedIndex: number;
  onSelectIndex: (index: number) => void;
  onActivate: (tool: ToolSummary) => void;
}

export function SearchView({
  query,
  onQueryChange,
  tools,
  showingRecents,
  selectedIndex,
  onSelectIndex,
  onActivate,
}: SearchViewProps) {
  return (
    <section className="search-view" aria-label="Tool search">
      <div className="search-view__input-row">
        <span className="search-view__prompt" aria-hidden="true">
          →
        </span>
        <input
          autoFocus
          className="search-view__input"
          type="search"
          placeholder="Search tools…"
          aria-label="Search tools"
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
        />
      </div>
      <div className="search-view__divider" />
      {showingRecents ? <div className="section-label">Recent</div> : null}
      <div className="search-view__results">
        {tools.map((tool, index) => (
          <ToolRow
            key={tool.id}
            tool={tool}
            selected={index === selectedIndex}
            onSelect={() => onSelectIndex(index)}
            onActivate={() => onActivate(tool)}
          />
        ))}
      </div>
    </section>
  );
}
