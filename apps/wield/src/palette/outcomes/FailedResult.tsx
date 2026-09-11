import type { Stage } from "../../lib/wield";
import "./outcomes.css";

export interface FailedResultProps {
  stage: Stage;
  detail: string;
  hint: string | null;
  onRetry: () => void;
  onCopyDetails: () => void;
}

export function FailedResult({
  stage,
  detail,
  hint,
  onRetry,
  onCopyDetails,
}: FailedResultProps) {
  return (
    <div className="outcome-card outcome-failed">
      <div className="outcome-failed__heading">
        <span className="outcome-failed__dot" aria-hidden="true" />
        <span>{stage} failed</span>
      </div>
      <div className="outcome-data">{detail}</div>
      {hint === null ? null : <div className="outcome-secondary">{hint}</div>}
      <div className="outcome-actions">
        <button type="button" className="outcome-action" onClick={onRetry}>
          Retry
        </button>
        <button type="button" className="outcome-action" onClick={onCopyDetails}>
          Copy details
        </button>
      </div>
    </div>
  );
}
