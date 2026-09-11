import type { HotkeyState } from "../lib/wield";
import "./preferences.css";

// The backend only reports whether binding succeeded, not the DE-confirmed
// trigger — this is the preferred trigger wield-portal::global_shortcuts
// requests. If the backend ever reports the actual confirmed trigger this
// should read from that instead.
const PREFERRED_TRIGGER = "Super+W";

export interface HotkeySectionProps {
  status: HotkeyState | null;
}

export function HotkeySection({ status }: HotkeySectionProps) {
  return (
    <section className="prefs-section">
      <h2>Hotkey</h2>
      {status === null || status.state === "Pending" ? (
        <p className="prefs-muted">Checking…</p>
      ) : status.state === "Registered" ? (
        <p>
          Global shortcut: <strong>{PREFERRED_TRIGGER}</strong> shows the palette.
        </p>
      ) : (
        <div>
          <p>
            The global shortcut isn&apos;t available. Bind this command to a key in your
            desktop environment&apos;s keyboard settings instead:
          </p>
          <div className="prefs-command-row">
            <code className="prefs-command">{status.fallback_command}</code>
            <button
              type="button"
              className="prefs-action"
              onClick={() => void navigator.clipboard.writeText(status.fallback_command)}
            >
              Copy
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
