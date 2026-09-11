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

import { cancelRun, listTools, runTool } from "./wield";

beforeEach(() => {
  invokeMock.mockReset();
  channelInstances.length = 0;
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
  const result = await runTool("image.convert", { input: "/x/a.png" }, (progress) =>
    seen.push(progress),
  );
  expect(result.outcome).toBe("Cancelled");
  const payload = invokeMock.mock.calls[0][1] as Record<string, unknown>;
  expect(payload.id).toBe("image.convert");
  expect(payload.args).toEqual({ input: "/x/a.png" });
  channelInstances.at(-1)?.onmessage?.("Started");
  expect(seen).toEqual(["Started"]);
});

test("cancelRun invokes cancel with the run id", async () => {
  invokeMock.mockResolvedValueOnce(true);
  expect(await cancelRun("abc")).toBe(true);
  expect(invokeMock).toHaveBeenCalledWith("cancel", { runId: "abc" });
});
