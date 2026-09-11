import type { Progress } from "../lib/wield";
import "./RunningView.css";

export interface RunningViewProps {
  toolTitle: string;
  progress: Progress | null;
}

export function RunningView({ toolTitle, progress }: RunningViewProps) {
  const message =
    progress !== null && typeof progress === "object" && "Message" in progress
      ? progress.Message
      : `${toolTitle}…`;
  const percent =
    progress !== null && typeof progress === "object" && "Percent" in progress
      ? Math.min(100, Math.max(0, progress.Percent))
      : null;

  return (
    <section className="running-view" aria-live="polite" aria-label="Tool running">
      <div className="running-view__status">
        <span className="spinner" aria-hidden="true" />
        <span>{message}</span>
      </div>
      {percent === null ? null : (
        <div className="progress-track" aria-label={`${percent}% complete`}>
          <div className="progress-fill" data-testid="progress-fill" style={{ width: `${percent}%` }} />
        </div>
      )}
      <div className="running-view__hint">esc to cancel</div>
    </section>
  );
}
