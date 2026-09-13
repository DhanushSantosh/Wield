import type { CapabilitiesReport, HotkeyState } from "../lib/wield";
import { HotkeySection } from "../preferences/HotkeySection";
import { PaletteBehaviorSection } from "../preferences/PaletteBehaviorSection";
import { SystemStatusSection } from "../preferences/SystemStatusSection";
import "./SettingsView.css";

export interface SettingsViewProps {
  hotkey: HotkeyState | null;
  report: CapabilitiesReport | null;
  onClose: () => void;
}

/**
 * Settings used to be a separate window, positioned and shown the same way
 * as the palette (see docs/testing.md for the layer-shell issues that came
 * with that: keyboard-exclusive with no dismiss path, no close affordance
 * at all, a rendering flake, and a sizing bug — a second window meant a
 * second copy of every one of those problems). Folding it into the
 * palette's own view state machine means it inherits the palette's already
 * solid show/hide/resize/keyboard handling for free instead of needing its
 * own copy of each fix.
 */
export function SettingsView({ hotkey, report, onClose }: SettingsViewProps) {
  return (
    <section className="settings-view" aria-label="Settings">
      <div className="settings-view__header">
        <span className="settings-view__title">Settings</span>
        <button
          type="button"
          className="settings-view__close"
          aria-label="Close settings"
          onClick={onClose}
        >
          ✕
        </button>
      </div>
      <div className="settings-view__body">
        <HotkeySection status={hotkey} />
        <PaletteBehaviorSection />
        <SystemStatusSection report={report} />
      </div>
    </section>
  );
}
