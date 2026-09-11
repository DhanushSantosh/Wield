import { beforeAll, beforeEach, expect, test } from "vitest";
import { getBlurToHide, setBlurToHide } from "./settings";

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

test("defaults to true when unset", () => {
  expect(getBlurToHide()).toBe(true);
});

test("setBlurToHide persists and getBlurToHide reflects it", () => {
  setBlurToHide(false);
  expect(getBlurToHide()).toBe(false);
  setBlurToHide(true);
  expect(getBlurToHide()).toBe(true);
});

test("corrupt storage falls back to the default", () => {
  window.localStorage.setItem("wield.settings.blurToHide", "not json");
  expect(getBlurToHide()).toBe(true);
});
