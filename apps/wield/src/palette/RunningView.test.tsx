import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { RunningView } from "./RunningView";

test("shows a percent bar when progress reports one", () => {
  render(<RunningView toolTitle="Convert image" progress={{ Percent: 62 }} />);
  expect(screen.getByTestId("progress-fill")).toHaveStyle({ width: "62%" });
});

test("shows the message text when progress is a Message", () => {
  render(<RunningView toolTitle="Convert image" progress={{ Message: "Encoding…" }} />);
  expect(screen.getByText("Encoding…")).toBeInTheDocument();
});

test("shows just the tool title with no bar before any progress arrives", () => {
  render(<RunningView toolTitle="Convert image" progress={null} />);
  expect(screen.getByText("Convert image…")).toBeInTheDocument();
  expect(screen.queryByTestId("progress-fill")).not.toBeInTheDocument();
});
