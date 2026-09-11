import { useState } from "react";
import { getBlurToHide, setBlurToHide } from "../lib/settings";
import "./preferences.css";

export function PaletteBehaviorSection() {
  const [blurToHide, setBlurToHideState] = useState(getBlurToHide);

  return (
    <section className="prefs-section">
      <h2>Palette behaviour</h2>
      <label className="prefs-checkbox-row">
        <input
          type="checkbox"
          checked={blurToHide}
          onChange={(event) => {
            const next = event.target.checked;
            setBlurToHide(next);
            setBlurToHideState(next);
          }}
        />
        Hide the palette when it loses focus
      </label>
    </section>
  );
}
