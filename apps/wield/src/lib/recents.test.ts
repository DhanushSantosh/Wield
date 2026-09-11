import { beforeAll, beforeEach, expect, test } from "vitest";
import { bumpRecent, getRecentIds } from "./recents";

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

test("bumpRecent inserts new ids at the front", () => {
  bumpRecent("a");
  bumpRecent("b");
  expect(getRecentIds()).toEqual(["b", "a"]);
});

test("bumpRecent moves an existing id to the front instead of duplicating", () => {
  bumpRecent("a");
  bumpRecent("b");
  bumpRecent("a");
  expect(getRecentIds()).toEqual(["a", "b"]);
});

test("bumpRecent caps at 8 entries", () => {
  for (let index = 0; index < 10; index += 1) {
    bumpRecent(`tool-${index}`);
  }
  expect(getRecentIds()).toHaveLength(8);
  expect(getRecentIds()[0]).toBe("tool-9");
});

test("getRecentIds returns an empty list when storage is empty or corrupt", () => {
  expect(getRecentIds()).toEqual([]);
  window.localStorage.setItem("wield.recents", "not json");
  expect(getRecentIds()).toEqual([]);
});
