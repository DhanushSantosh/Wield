import "./RunningView.css";

export interface RunningViewProps {
  toolTitle: string;
  message: string | null;
  percent: number | null;
}

export function RunningView({ toolTitle, message, percent }: RunningViewProps) {
  const label = message ?? `${toolTitle}…`;
  const clamped = percent === null ? null : Math.min(100, Math.max(0, percent));

  return (
    <section className="running-view" aria-live="polite" aria-label="Tool running">
      <div className="running-view__status">
        <span className="spinner" aria-hidden="true" />
        <span>{label}</span>
      </div>
      {clamped === null ? null : (
        <div className="progress-track" aria-label={`${clamped}% complete`}>
          <div className="progress-fill" data-testid="progress-fill" style={{ width: `${clamped}%` }} />
        </div>
      )}
      <div className="running-view__hint">esc to cancel</div>
    </section>
  );
}
