import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { test, expect, vi, beforeEach } from "vitest";

const configureHotkeyMock = vi.fn();
vi.mock("../lib/wield", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/wield")>();
  return {
    ...original,
    configureHotkey: (...args: unknown[]) => configureHotkeyMock(...args),
  };
});

import { HotkeySection } from "./HotkeySection";

beforeEach(() => {
  configureHotkeyMock.mockReset().mockResolvedValue(undefined);
});

test("shows a loading state before status arrives", () => {
  render(<HotkeySection status={null} />);
  expect(screen.getByText(/checking/i)).toBeInTheDocument();
});

test("shows a loading state while pending", () => {
  render(<HotkeySection status={{ state: "Pending" }} />);
  expect(screen.getByText(/checking/i)).toBeInTheDocument();
});

test("shows the bound shortcut and a working Change shortcut button when registered", async () => {
  render(<HotkeySection status={{ state: "Registered" }} />);
  expect(screen.getByText(/Super\+W/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: /change shortcut/i }));
  expect(configureHotkeyMock).toHaveBeenCalledWith();
});

test("shows an error if changing the shortcut fails", async () => {
  configureHotkeyMock.mockRejectedValueOnce(new Error("no active hotkey session to reconfigure"));
  render(<HotkeySection status={{ state: "Registered" }} />);
  await userEvent.click(screen.getByRole("button", { name: /change shortcut/i }));
  await waitFor(() =>
    expect(screen.getByText(/no active hotkey session to reconfigure/i)).toBeInTheDocument(),
  );
});

test("shows the fallback command with a working copy button when unavailable, and no Change shortcut button", async () => {
  Object.assign(navigator, { clipboard: { writeText: vi.fn() } });
  render(<HotkeySection status={{ state: "Unavailable", fallback_command: "wield-app" }} />);
  expect(screen.getByText("wield-app")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /change shortcut/i })).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: /copy/i }));
  expect(navigator.clipboard.writeText).toHaveBeenCalledWith("wield-app");
});
