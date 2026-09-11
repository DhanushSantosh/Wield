import { render, screen } from "@testing-library/react";
import { beforeEach, test, expect, vi } from "vitest";

const hotkeyStatusMock = vi.fn();
const capabilitiesMock = vi.fn();
vi.mock("./lib/wield", async (importOriginal) => {
  const original = await importOriginal<typeof import("./lib/wield")>();
  return {
    ...original,
    hotkeyStatus: () => hotkeyStatusMock(),
    capabilities: () => capabilitiesMock(),
  };
});

import Preferences from "./Preferences";

beforeEach(() => {
  hotkeyStatusMock.mockReset().mockResolvedValue({ state: "Registered" });
  capabilitiesMock.mockReset().mockResolvedValue({ binaries: {}, portals: {}, tools: [] });
});

test("renders all three sections once data loads", async () => {
  render(<Preferences />);
  expect(await screen.findByText(/Super\+W/)).toBeInTheDocument();
  expect(
    screen.getByRole("checkbox", { name: /hide the palette when it loses focus/i }),
  ).toBeInTheDocument();
  expect(screen.getByText(/system status/i)).toBeInTheDocument();
});
