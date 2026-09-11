import "./outcomes.css";

export interface ReportResultProps {
  title: string;
  lines: string[];
}

export function ReportResult({ title, lines }: ReportResultProps) {
  return (
    <div className="outcome-card">
      <h2 className="outcome-title">{title}</h2>
      <div className="outcome-report">
        {lines.map((line, index) => (
          <div key={`${index}-${line}`}>{line}</div>
        ))}
      </div>
    </div>
  );
}
