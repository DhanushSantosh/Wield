import { beforeEach, expect, test, vi } from "vitest";

const invokeMock = vi.fn();
const channelInstances: Array<{
  onmessage: ((payload: unknown) => void) | undefined;
}> = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  Channel: class {
    onmessage: ((payload: unknown) => void) | undefined;

    constructor() {
      channelInstances.push(this);
    }
  },
}));

const listenMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

import {
  cancelRun,
  capabilities,
  hidePalette,
  resizePalette,
  hotkeyStatus,
  configureHotkey,
  listTools,
  onSelectTool,
  runTool,
} from "./wield";

beforeEach(() => {
  invokeMock.mockReset();
  channelInstances.length = 0;
  listenMock.mockReset();
});

test("listTools passes the query through, defaulting to null", async () => {
  invokeMock.mockResolvedValueOnce([]);
  await listTools();
  expect(invokeMock).toHaveBeenCalledWith("list_tools", { query: null });

  invokeMock.mockResolvedValueOnce([]);
  await listTools("convert");
  expect(invokeMock).toHaveBeenCalledWith("list_tools", { query: "convert" });
});

test("runTool wires a Channel and forwards progress events to onProgress", async () => {
  invokeMock.mockResolvedValueOnce({ run_id: "abc", outcome: "Cancelled" });
  const seen: unknown[] = [];
  const result = await runTool(
    "image.convert",
    { input: "/x/a.png" },
    (progress) => seen.push(progress),
    "known-run",
  );
  expect(result.outcome).toBe("Cancelled");
  const payload = invokeMock.mock.calls[0][1] as Record<string, unknown>;
  expect(payload.id).toBe("image.convert");
  expect(payload.args).toEqual({ input: "/x/a.png" });
  expect(payload.runId).toBe("known-run");
  channelInstances.at(-1)?.onmessage?.("Started");
  expect(seen).toEqual(["Started"]);
});

test("cancelRun invokes cancel with the run id", async () => {
  invokeMock.mockResolvedValueOnce(true);
  expect(await cancelRun("abc")).toBe(true);
  expect(invokeMock).toHaveBeenCalledWith("cancel", { runId: "abc" });
});

test("hidePalette invokes the palette hide command", async () => {
  invokeMock.mockResolvedValueOnce(undefined);
  await hidePalette();
  expect(invokeMock).toHaveBeenCalledWith("hide_palette");
});

test("resizePalette invokes resize_palette with the height", async () => {
  invokeMock.mockResolvedValueOnce(undefined);
  await resizePalette(240);
  expect(invokeMock).toHaveBeenCalledWith("resize_palette", { height: 240 });
});

test("hotkeyStatus invokes hotkey_status", async () => {
  invokeMock.mockResolvedValueOnce({ state: "Registered" });
  expect(await hotkeyStatus()).toEqual({ state: "Registered" });
  expect(invokeMock).toHaveBeenCalledWith("hotkey_status");
});

test("configureHotkey invokes configure_hotkey", async () => {
  invokeMock.mockResolvedValueOnce(undefined);
  await configureHotkey();
  expect(invokeMock).toHaveBeenCalledWith("configure_hotkey");
});

test("capabilities invokes capabilities", async () => {
  const report = { binaries: { magick: true }, portals: { Screenshot: 2 }, tools: [] };
  invokeMock.mockResolvedValueOnce(report);
  expect(await capabilities()).toEqual(report);
  expect(invokeMock).toHaveBeenCalledWith("capabilities");
});

test("onSelectTool forwards the event payload and returns an unsubscribe fn", async () => {
  let captured: ((event: { payload: string }) => void) | undefined;
  const unlistenMock = vi.fn();
  listenMock.mockImplementation((_name: string, cb: typeof captured) => {
    captured = cb;
    return Promise.resolve(unlistenMock);
  });
  const handler = vi.fn();
  const unsubscribe = await onSelectTool(handler);
  captured?.({ payload: "image.convert" });
  expect(handler).toHaveBeenCalledWith("image.convert");
  expect(listenMock).toHaveBeenCalledWith("tray://select-tool", expect.any(Function));
  unsubscribe();
  expect(unlistenMock).toHaveBeenCalled();
});
