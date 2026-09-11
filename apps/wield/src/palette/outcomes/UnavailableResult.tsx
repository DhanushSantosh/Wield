import "./outcomes.css";

export interface UnavailableResultProps {
  reason: string;
  fix: string | null;
}

export function UnavailableResult({ reason, fix }: UnavailableResultProps) {
  return (
    <div className="outcome-card outcome-unavailable">
      <div className="outcome-label">Not set up yet</div>
      <div>{reason}</div>
      {fix === null ? null : <div className="outcome-secondary">{fix}</div>}
    </div>
  );
}
