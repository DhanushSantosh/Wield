const TEETH_ANGLES = [0, 45, 90, 135, 180, 225, 270, 315];

/**
 * A simple gear built from robust primitives (a circle hub plus rotated
 * rounded rects for teeth) rather than one hand-drawn path — matches
 * CategoryIcon's preference for shapes that are easy to get right over
 * a single intricate curve.
 */
export function SettingsIcon() {
  return (
    <svg className="settings-icon" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <circle cx="10" cy="10" r="3" stroke="currentColor" strokeWidth="1.4" />
      {TEETH_ANGLES.map((angle) => (
        <rect
          key={angle}
          x="9.1"
          y="1.4"
          width="1.8"
          height="3.1"
          rx="0.6"
          fill="currentColor"
          transform={`rotate(${angle} 10 10)`}
        />
      ))}
    </svg>
  );
}
