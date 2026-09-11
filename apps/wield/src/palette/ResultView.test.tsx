import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeAll, expect, test, vi } from "vitest";
import { ResultView } from "./ResultView";

const revealMock = vi.fn();
vi.mock("@tauri-apps/plugin-opener", () => ({
  revealItemInDir: (...args: unknown[]) => revealMock(...args),
}));

const clipboardWrite = vi.fn().mockResolvedValue(undefined);
beforeAll(() => {
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: clipboardWrite },
  });
});

test("renders and copies a Value outcome", async () => {
  render(
    <ResultView
      outcome={{ Value: { kind: "Text", data: "copied text" } }}
      onRunAgain={vi.fn()}
      onRetry={vi.fn()}
    />,
  );
  expect(screen.getByText("copied text")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Copy value" }));
  expect(clipboardWrite).toHaveBeenCalledWith("copied text");
});

test("renders and reveals a File outcome", async () => {
  render(
    <ResultView
      outcome={{ File: { path: "/tmp/output.png" } }}
      onRunAgain={vi.fn()}
      onRetry={vi.fn()}
    />,
  );
  expect(screen.getByText("/tmp/output.png")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Open folder" }));
  expect(revealMock).toHaveBeenCalledWith("/tmp/output.png");
});

test("renders a Report outcome", () => {
  render(
    <ResultView
      outcome={{ Report: { title: "Summary", lines: ["Everything passed"] } }}
      onRunAgain={vi.fn()}
      onRetry={vi.fn()}
    />,
  );
  expect(screen.getByText("Everything passed")).toBeInTheDocument();
});

test("renders an Unavailable outcome", () => {
  render(
    <ResultView
      outcome={{ Unavailable: { reason: "Portal missing", fix: null } }}
      onRunAgain={vi.fn()}
      onRetry={vi.fn()}
    />,
  );
  expect(screen.getByText("Portal missing")).toBeInTheDocument();
});

test("renders a Failed outcome and wires retry", async () => {
  const onRetry = vi.fn();
  render(
    <ResultView
      outcome={{ Failed: { stage: "Command", detail: "exit 1", hint: null } }}
      onRunAgain={vi.fn()}
      onRetry={onRetry}
    />,
  );
  expect(screen.getByText("exit 1")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(onRetry).toHaveBeenCalled();
});

test("renders nothing for a Cancelled outcome", () => {
  const { container } = render(
    <ResultView outcome="Cancelled" onRunAgain={vi.fn()} onRetry={vi.fn()} />,
  );
  expect(container).toBeEmptyDOMElement();
});
