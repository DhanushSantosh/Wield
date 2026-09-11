import "./outcomes.css";

export interface FileResultProps {
  path: string;
  onOpenFolder: () => void;
  onRunAgain: () => void;
}

export function FileResult({ path, onOpenFolder, onRunAgain }: FileResultProps) {
  return (
    <div className="outcome-card">
      <div className="outcome-label">Saved file</div>
      <div className="outcome-data">{path}</div>
      <div className="outcome-actions">
        <button type="button" className="outcome-action" onClick={onOpenFolder}>
          Open folder
        </button>
        <button type="button" className="outcome-action" onClick={onRunAgain}>
          Run again
        </button>
      </div>
    </div>
  );
}
