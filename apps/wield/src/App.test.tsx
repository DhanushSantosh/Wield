import { render, screen } from "@testing-library/react";
import { vi } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({ name: "Wield", version: "0.1.0" }),
}));

test("renders the Wield heading", async () => {
  render(<App />);
  expect(await screen.findByRole("heading", { name: "Wield" })).toBeInTheDocument();
});
