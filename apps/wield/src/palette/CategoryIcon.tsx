import type { ReactElement } from "react";
import type { Category } from "../lib/wield";

/**
 * One simple line-icon per tool category, so each row in the results list
 * has something more meaningful than an empty placeholder square. Keyed by
 * category (not tool id) so any future tool automatically gets a sensible
 * icon just by being categorized correctly — no per-tool icon to maintain.
 */
export function CategoryIcon({ category }: { category: Category }) {
  return (
    <svg
      className="category-icon"
      viewBox="0 0 20 20"
      fill="none"
      aria-hidden="true"
    >
      {ICONS[category]}
    </svg>
  );
}

const ICONS: Record<Category, ReactElement> = {
  // A droplet — matches "Pick a colour", the category's one tool today.
  Capture: (
    <path
      d="M10 3C10 3 4.75 9.75 4.75 13.25C4.75 16.15 7.1 18.25 10 18.25C12.9 18.25 15.25 16.15 15.25 13.25C15.25 9.75 10 3 10 3Z"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinejoin="round"
    />
  ),
  // Two opposing arrows — a standard "convert/exchange" glyph.
  Convert: (
    <path
      d="M4 7H16M16 7L12.75 4M16 7L12.75 10M16 13H4M4 13L7.25 10M4 13L7.25 16"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  ),
  // A simple window — a generic default for future desktop-utility tools.
  Desktop: (
    <path
      d="M3.5 5.5C3.5 4.67 4.17 4 5 4H15C15.83 4 16.5 4.67 16.5 5.5V14.5C16.5 15.33 15.83 16 15 16H5C4.17 16 3.5 15.33 3.5 14.5V5.5Z M3.5 7.5H16.5"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinejoin="round"
    />
  ),
};
