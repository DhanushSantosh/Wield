import { renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useKeyboardNav } from "./useKeyboardNav";

function fire(key: string) {
  window.dispatchEvent(new KeyboardEvent("keydown", { key, cancelable: true }));
}

test("arrow keys move selection, clamped to the list bounds", () => {
  const onSelectIndex = vi.fn();
  renderHook(() =>
    useKeyboardNav({
      itemCount: 3,
      selectedIndex: 1,
      onSelectIndex,
      onActivate: vi.fn(),
      onEscape: vi.fn(),
    }),
  );
  fire("ArrowDown");
  expect(onSelectIndex).toHaveBeenCalledWith(2);
  fire("ArrowUp");
  expect(onSelectIndex).toHaveBeenCalledWith(0);
});

test("ArrowDown at the last item does not go out of bounds", () => {
  const onSelectIndex = vi.fn();
  renderHook(() =>
    useKeyboardNav({
      itemCount: 2,
      selectedIndex: 1,
      onSelectIndex,
      onActivate: vi.fn(),
      onEscape: vi.fn(),
    }),
  );
  fire("ArrowDown");
  expect(onSelectIndex).toHaveBeenCalledWith(1);
});

test("Enter and Escape call their handlers", () => {
  const onActivate = vi.fn();
  const onEscape = vi.fn();
  renderHook(() =>
    useKeyboardNav({
      itemCount: 1,
      selectedIndex: 0,
      onSelectIndex: vi.fn(),
      onActivate,
      onEscape,
    }),
  );
  fire("Enter");
  fire("Escape");
  expect(onActivate).toHaveBeenCalled();
  expect(onEscape).toHaveBeenCalled();
});
