import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { UnavailableResult } from "./UnavailableResult";

test("renders setup guidance without presenting it as an error", () => {
  render(<UnavailableResult reason="Portal unavailable" fix="Enable the portal" />);
  expect(screen.getByText("Portal unavailable")).toBeInTheDocument();
  expect(screen.getByText("Enable the portal")).toBeInTheDocument();
});
