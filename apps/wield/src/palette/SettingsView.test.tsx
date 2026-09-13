import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { SettingsView } from "./SettingsView";

test("renders all three sections", () => {
  render(
    <SettingsView
      hotkey={{ state: "Registered" }}
      report={{ binaries: {}, portals: {}, tools: [] }}
      onClose={vi.fn()}
    />,
  );
  expect(screen.getByText(/Super\+Space/)).toBeInTheDocument();
  expect(
    screen.getByRole("checkbox", { name: /hide the palette when it loses focus/i }),
  ).toBeInTheDocument();
  expect(screen.getByText(/system status/i)).toBeInTheDocument();
});

test("the close button calls onClose", async () => {
  const onClose = vi.fn();
  render(
    <SettingsView
      hotkey={{ state: "Registered" }}
      report={{ binaries: {}, portals: {}, tools: [] }}
      onClose={onClose}
    />,
  );
  await userEvent.click(screen.getByRole("button", { name: "Close settings" }));
  expect(onClose).toHaveBeenCalledTimes(1);
});
