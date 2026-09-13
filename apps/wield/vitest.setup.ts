import "@testing-library/jest-dom/vitest";

// jsdom doesn't implement ResizeObserver. A no-op stub is enough for every
// test that doesn't care about resize behavior specifically; a test file
// that does (App.test.tsx) overrides this with a controllable fake via
// vi.stubGlobal, scoped to just that file.
if (typeof globalThis.ResizeObserver === "undefined") {
  class NoopResizeObserver implements ResizeObserver {
    observe() {
      // no-op: this environment never fires resize callbacks on its own.
    }
    unobserve() {}
    disconnect() {}
  }
  globalThis.ResizeObserver = NoopResizeObserver;
}
