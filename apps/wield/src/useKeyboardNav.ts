import { useEffect } from "react";

export interface KeyboardNavOptions {
  itemCount: number;
  selectedIndex: number;
  onSelectIndex: (index: number) => void;
  onActivate: () => void;
  onEscape: () => void;
}

export function useKeyboardNav({
  itemCount,
  selectedIndex,
  onSelectIndex,
  onActivate,
  onEscape,
}: KeyboardNavOptions): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      switch (event.key) {
        case "ArrowDown":
          event.preventDefault();
          if (itemCount > 0) {
            onSelectIndex(Math.min(selectedIndex + 1, itemCount - 1));
          }
          break;
        case "ArrowUp":
          event.preventDefault();
          if (itemCount > 0) {
            onSelectIndex(Math.max(selectedIndex - 1, 0));
          }
          break;
        case "Enter":
          event.preventDefault();
          onActivate();
          break;
        case "Escape":
          event.preventDefault();
          onEscape();
          break;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [itemCount, onActivate, onEscape, onSelectIndex, selectedIndex]);
}
