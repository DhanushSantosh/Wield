import { useEffect, useState } from "react";
import { capabilities, hotkeyStatus, type CapabilitiesReport, type HotkeyState } from "./lib/wield";
import { HotkeySection } from "./preferences/HotkeySection";
import { PaletteBehaviorSection } from "./preferences/PaletteBehaviorSection";
import { SystemStatusSection } from "./preferences/SystemStatusSection";
import "./preferences/preferences.css";

export default function Preferences() {
  const [hotkey, setHotkey] = useState<HotkeyState | null>(null);
  const [report, setReport] = useState<CapabilitiesReport | null>(null);

  useEffect(() => {
    void hotkeyStatus().then(setHotkey);
  }, []);

  useEffect(() => {
    void capabilities().then(setReport);
  }, []);

  return (
    <div className="prefs-page">
      <h1>Wield Preferences</h1>
      <HotkeySection status={hotkey} />
      <PaletteBehaviorSection />
      <SystemStatusSection report={report} />
    </div>
  );
}
