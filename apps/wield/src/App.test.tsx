import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeAll, beforeEach, expect, test, vi } from "vitest";
import type { RunResult, ToolSummary } from "./lib/wield";

const listToolsMock = vi.fn();
const runToolMock = vi.fn();
const cancelRunMock = vi.fn();
const hidePaletteMock = vi.fn();
const resizePaletteMock = vi.fn();
const hotkeyStatusMock = vi.fn();
const capabilitiesMock = vi.fn();
let selectToolHandler: ((toolId: string) => void) | undefined;
let openSettingsHandler: (() => void) | undefined;

vi.mock("./lib/wield", async (importOriginal) => {
  const original = await importOriginal<typeof import("./lib/wield")>();
  return {
    ...original,
    listTools: (...args: unknown[]) => listToolsMock(...args),
    runTool: (...args: unknown[]) => runToolMock(...args),
    cancelRun: (...args: unknown[]) => cancelRunMock(...args),
    hidePalette: (...args: unknown[]) => hidePaletteMock(...args),
    resizePalette: (...args: unknown[]) => resizePaletteMock(...args),
    hotkeyStatus: () => hotkeyStatusMock(),
    capabilities: () => capabilitiesMock(),
    onSelectTool: async (handler: (toolId: string) => void) => {
      selectToolHandler = handler;
      return () => {
        selectToolHandler = undefined;
      };
    },
    onOpenSettings: async (handler: () => void) => {
      openSettingsHandler = handler;
      return () => {
        openSettingsHandler = undefined;
      };
    },
  };
});

vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

import App from "./App";

const colorTool: ToolSummary = {
  id: "color.pick",
  title: "Pick a color",
  keywords: ["color"],
  category: "Capture",
  args: [],
  available: true,
  reason: null,
};

const convertTool: ToolSummary = {
  id: "image.convert",
  title: "Convert image",
  keywords: ["image", "convert"],
  category: "Convert",
  args: [
    {
      name: "format",
      label: "Format",
      help: null,
      arg_type: { Enum: { options: ["png", "webp"] } },
      default: { Str: "png" },
      required: true,
      when: null,
    },
  ],
  available: true,
  reason: null,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

const storedValues = new Map<string, string>();
const storage: Storage = {
  get length() {
    return storedValues.size;
  },
  clear: () => storedValues.clear(),
  getItem: (key) => storedValues.get(key) ?? null,
  key: (index) => [...storedValues.keys()][index] ?? null,
  removeItem: (key) => {
    storedValues.delete(key);
  },
  setItem: (key, value) => {
    storedValues.set(key, value);
  },
};

beforeAll(() => {
  Object.defineProperty(window, "localStorage", { configurable: true, value: storage });
});

let resizeObserverCallback: ResizeObserverCallback | undefined;

class FakeResizeObserver implements ResizeObserver {
  constructor(callback: ResizeObserverCallback) {
    resizeObserverCallback = callback;
  }
  observe() {}
  unobserve() {}
  disconnect() {
    resizeObserverCallback = undefined;
  }
}

beforeEach(() => {
  listToolsMock.mockReset().mockResolvedValue([colorTool, convertTool]);
  runToolMock.mockReset();
  cancelRunMock.mockReset().mockResolvedValue(true);
  hidePaletteMock.mockReset().mockResolvedValue(undefined);
  resizePaletteMock.mockReset().mockResolvedValue(undefined);
  hotkeyStatusMock.mockReset().mockResolvedValue({ state: "Registered" });
  capabilitiesMock.mockReset().mockResolvedValue({ binaries: {}, portals: {}, tools: [] });
  resizeObserverCallback = undefined;
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  window.localStorage.clear();
});

test("starts in search and loads the tool list", async () => {
  render(<App />);
  expect(screen.getByRole("searchbox", { name: "Search tools" })).toBeInTheDocument();
  expect(await screen.findByText("Pick a color")).toBeInTheDocument();
  expect(listToolsMock).toHaveBeenCalledWith(undefined);
});

test("typing a query renders the backend-ranked results in order", async () => {
  listToolsMock.mockImplementation((query?: string) =>
    Promise.resolve(query === "image" ? [convertTool, colorTool] : [colorTool, convertTool]),
  );
  render(<App />);
  await userEvent.type(screen.getByRole("searchbox", { name: "Search tools" }), "image");
  await waitFor(() => expect(listToolsMock).toHaveBeenLastCalledWith("image"));
  // Excludes the settings gear button, also a role="button" in this same
  // input row but not one of the ranked result rows under test here.
  const rows = screen
    .getAllByRole("button")
    .filter((row) => row.getAttribute("aria-label") !== "Open settings");
  expect(rows.map((row) => row.textContent)).toEqual([
    "Convert imageConvert",
    "Pick a colorCapture",
  ]);
});

test("an available no-argument tool runs immediately and shows its result", async () => {
  runToolMock.mockResolvedValue({
    run_id: "run-1",
    outcome: { Value: { kind: "Text", data: "done" } },
  });
  render(<App />);
  await userEvent.click(await screen.findByText("Pick a color"));
  expect(runToolMock).toHaveBeenCalledWith(
    "color.pick",
    {},
    expect.any(Function),
    expect.any(String),
  );
  expect(await screen.findByText("done")).toBeInTheDocument();
});

test("an argument tool opens its form and streams progress before the result", async () => {
  const run = deferred<RunResult>();
  runToolMock.mockReturnValue(run.promise);
  render(<App />);
  await userEvent.click(await screen.findByText("Convert image"));
  expect(screen.getByRole("heading", { name: "Convert image" })).toBeInTheDocument();
  await userEvent.selectOptions(screen.getByLabelText("Format"), "webp");
  await userEvent.click(screen.getByRole("button", { name: "Convert image" }));
  const onProgress = runToolMock.mock.calls[0][2] as (progress: unknown) => void;
  act(() => onProgress({ Message: "Converting…" }));
  expect(await screen.findByText("Converting…")).toBeInTheDocument();
  await act(async () =>
    run.resolve({ run_id: "run-2", outcome: { File: { path: "/tmp/out.webp" } } }),
  );
  expect(await screen.findByText("/tmp/out.webp")).toBeInTheDocument();
});

test("Escape cancels a running tool and waits for Cancelled before returning to search", async () => {
  const run = deferred<RunResult>();
  runToolMock.mockReturnValue(run.promise);
  render(<App />);
  await userEvent.click(await screen.findByText("Pick a color"));
  expect(screen.getByLabelText("Tool running")).toBeInTheDocument();
  fireEvent.keyDown(window, { key: "Escape" });
  const runId = runToolMock.mock.calls[0][3] as string;
  expect(cancelRunMock).toHaveBeenCalledWith(runId);
  expect(screen.getByLabelText("Tool running")).toBeInTheDocument();
  await act(async () => run.resolve({ run_id: runId, outcome: "Cancelled" }));
  expect(await screen.findByRole("searchbox", { name: "Search tools" })).toBeInTheDocument();
  expect(screen.queryByLabelText("Tool result")).not.toBeInTheDocument();
});

test("Retry reruns a failure immediately with the exact same arguments", async () => {
  runToolMock
    .mockResolvedValueOnce({
      run_id: "run-failed",
      outcome: { Failed: { stage: "Command", detail: "boom", hint: null } },
    })
    .mockResolvedValueOnce({
      run_id: "run-retry",
      outcome: { File: { path: "/tmp/retried.png" } },
    });
  render(<App />);
  await userEvent.click(await screen.findByText("Convert image"));
  await userEvent.selectOptions(screen.getByLabelText("Format"), "webp");
  await userEvent.click(screen.getByRole("button", { name: "Convert image" }));
  await userEvent.click(await screen.findByRole("button", { name: "Retry" }));
  expect(screen.queryByRole("heading", { name: "Convert image" })).not.toBeInTheDocument();
  expect(runToolMock).toHaveBeenNthCalledWith(
    2,
    "image.convert",
    { format: "webp" },
    expect.any(Function),
    expect.any(String),
  );
  expect(await screen.findByText("/tmp/retried.png")).toBeInTheDocument();
});

test("Escape steps back from forms and normal results", async () => {
  render(<App />);
  await userEvent.click(await screen.findByText("Convert image"));
  fireEvent.keyDown(screen.getByRole("heading", { name: "Convert image" }), { key: "Escape" });
  expect(await screen.findByRole("searchbox", { name: "Search tools" })).toBeInTheDocument();

  runToolMock.mockResolvedValue({
    run_id: "run-4",
    outcome: { Value: { kind: "Text", data: "done" } },
  });
  await userEvent.click(screen.getByText("Pick a color"));
  expect(await screen.findByText("done")).toBeInTheDocument();
  fireEvent.keyDown(window, { key: "Escape" });
  expect(await screen.findByRole("searchbox", { name: "Search tools" })).toBeInTheDocument();
});

test("Run again retains values and marks the next result for form-style Escape", async () => {
  runToolMock.mockResolvedValue({
    run_id: "run-5",
    outcome: { File: { path: "/tmp/out.webp" } },
  });
  render(<App />);
  await userEvent.click(await screen.findByText("Convert image"));
  await userEvent.selectOptions(screen.getByLabelText("Format"), "webp");
  await userEvent.click(screen.getByRole("button", { name: "Convert image" }));
  await userEvent.click(await screen.findByRole("button", { name: "Run again" }));
  expect(screen.getByLabelText("Format")).toHaveValue("webp");
  await userEvent.click(screen.getByRole("button", { name: "Convert image" }));
  expect(await screen.findByText("/tmp/out.webp")).toBeInTheDocument();
  expect(runToolMock).toHaveBeenCalledTimes(2);
  fireEvent.keyDown(window, { key: "Escape" });
  await waitFor(() => expect(screen.getByLabelText("Format")).toHaveValue("webp"));
});

test("an unavailable selected row shows its reason and Enter does nothing", async () => {
  const unavailable = {
    ...convertTool,
    available: false,
    reason: "ImageMagick is not installed",
  };
  listToolsMock.mockResolvedValue([unavailable]);
  render(<App />);
  expect(await screen.findByText("ImageMagick is not installed")).toBeInTheDocument();
  fireEvent.keyDown(window, { key: "Enter" });
  expect(runToolMock).not.toHaveBeenCalled();
  expect(screen.getByRole("searchbox", { name: "Search tools" })).toBeInTheDocument();
});

test("top-level Escape and window blur hide the palette", async () => {
  render(<App />);
  await screen.findByText("Pick a color");
  fireEvent.keyDown(window, { key: "Escape" });
  fireEvent(window, new Event("blur"));
  expect(hidePaletteMock).toHaveBeenCalledTimes(2);
});

test("blur does not hide the palette when the setting is off", async () => {
  const { setBlurToHide } = await import("./lib/settings");
  setBlurToHide(false);
  render(<App />);
  await screen.findByText("Pick a color");
  fireEvent(window, new Event("blur"));
  expect(hidePaletteMock).not.toHaveBeenCalled();
  setBlurToHide(true);
});

test("a tray tool-selection event opens the form for an argument tool", async () => {
  render(<App />);
  await screen.findByText("Pick a color");
  await act(async () => selectToolHandler?.("image.convert"));
  expect(await screen.findByRole("heading", { name: "Convert image" })).toBeInTheDocument();
});

test("a tray tool-selection event runs a no-argument tool immediately", async () => {
  runToolMock.mockResolvedValue({
    run_id: "run-tray",
    outcome: { Value: { kind: "Text", data: "done" } },
  });
  render(<App />);
  await screen.findByText("Pick a color");
  await act(async () => selectToolHandler?.("color.pick"));
  expect(await screen.findByText("done")).toBeInTheDocument();
});

test("a tray tool-selection event for an unavailable tool does nothing", async () => {
  listToolsMock.mockResolvedValue([
    { ...convertTool, available: false, reason: "ImageMagick is not installed" },
  ]);
  render(<App />);
  await screen.findByText("ImageMagick is not installed");
  await act(async () => selectToolHandler?.("image.convert"));
  expect(runToolMock).not.toHaveBeenCalled();
  expect(screen.getByRole("searchbox", { name: "Search tools" })).toBeInTheDocument();
});

test("reports the shell's rendered height to resizePalette whenever it changes", async () => {
  render(<App />);
  await screen.findByText("Pick a color");
  expect(resizeObserverCallback).toBeDefined();

  const fakeEntry = { contentRect: { height: 246 } } as ResizeObserverEntry;
  act(() => resizeObserverCallback?.([fakeEntry], {} as ResizeObserver));
  expect(resizePaletteMock).toHaveBeenCalledWith(246);
});

test("the settings gear opens settings, and its close button returns to search", async () => {
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: "Open settings" }));
  expect(await screen.findByText(/Super\+Space/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Close settings" }));
  expect(await screen.findByRole("searchbox", { name: "Search tools" })).toBeInTheDocument();
});

test("Escape closes settings back to search", async () => {
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: "Open settings" }));
  await screen.findByText(/Super\+Space/);
  fireEvent.keyDown(window, { key: "Escape" });
  expect(await screen.findByRole("searchbox", { name: "Search tools" })).toBeInTheDocument();
});

test("a tray open-settings event opens settings the same way the gear does", async () => {
  render(<App />);
  await screen.findByText("Pick a color");
  await act(async () => openSettingsHandler?.());
  expect(await screen.findByText(/Super\+Space/)).toBeInTheDocument();
});
