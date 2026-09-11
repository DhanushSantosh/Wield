import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { ReportResult } from "./ReportResult";

test("renders a report title and every line", () => {
  render(<ReportResult title="Inspection" lines={["First", "Second"]} />);
  expect(screen.getByRole("heading", { name: "Inspection" })).toBeInTheDocument();
  expect(screen.getByText("First")).toBeInTheDocument();
  expect(screen.getByText("Second")).toBeInTheDocument();
});
