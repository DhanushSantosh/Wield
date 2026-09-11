import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { test, expect, vi } from "vitest";
import { HotkeySection } from "./HotkeySection";

test("shows a loading state before status arrives", () => {
  render(<HotkeySection status={null} />);
  expect(screen.getByText(/checking/i)).toBeInTheDocument();
});

test("shows a loading state while pending", () => {
  render(<HotkeySection status={{ state: "Pending" }} />);
  expect(screen.getByText(/checking/i)).toBeInTheDocument();
});

test("shows the bound shortcut when registered", () => {
  render(<HotkeySection status={{ state: "Registered" }} />);
  expect(screen.getByText(/Super\+W/)).toBeInTheDocument();
});

test("shows the fallback command with a working copy button when unavailable", async () => {
  Object.assign(navigator, { clipboard: { writeText: vi.fn() } });
  render(<HotkeySection status={{ state: "Unavailable", fallback_command: "wield-app" }} />);
  expect(screen.getByText("wield-app")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: /copy/i }));
  expect(navigator.clipboard.writeText).toHaveBeenCalledWith("wield-app");
});
