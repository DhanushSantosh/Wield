import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeAll, beforeEach, test, expect } from "vitest";
import { PaletteBehaviorSection } from "./PaletteBehaviorSection";
import { getBlurToHide } from "../lib/settings";

const values = new Map<string, string>();
const storage: Storage = {
  get length() {
    return values.size;
  },
  clear: () => values.clear(),
  getItem: (key) => values.get(key) ?? null,
  key: (index) => [...values.keys()][index] ?? null,
  removeItem: (key) => {
    values.delete(key);
  },
  setItem: (key, value) => {
    values.set(key, value);
  },
};

beforeAll(() => {
  Object.defineProperty(window, "localStorage", { configurable: true, value: storage });
});
beforeEach(() => window.localStorage.clear());

test("checkbox reflects and updates the blur-to-hide setting", async () => {
  render(<PaletteBehaviorSection />);
  const checkbox = screen.getByRole("checkbox", { name: /hide the palette when it loses focus/i });
  expect(checkbox).toBeChecked();
  await userEvent.click(checkbox);
  expect(checkbox).not.toBeChecked();
  expect(getBlurToHide()).toBe(false);
});
