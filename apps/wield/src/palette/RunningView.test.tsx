import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { RunningView } from "./RunningView";

test("shows a percent bar when percent is set", () => {
  render(<RunningView toolTitle="Convert image" message={null} percent={62} />);
  expect(screen.getByTestId("progress-fill")).toHaveStyle({ width: "62%" });
});

test("shows the message text when one is set", () => {
  render(<RunningView toolTitle="Convert image" message="Encoding…" percent={null} />);
  expect(screen.getByText("Encoding…")).toBeInTheDocument();
});

test("shows just the tool title with no bar before any progress arrives", () => {
  render(<RunningView toolTitle="Convert image" message={null} percent={null} />);
  expect(screen.getByText("Convert image…")).toBeInTheDocument();
  expect(screen.queryByTestId("progress-fill")).not.toBeInTheDocument();
});

test("shows a batch's status message together with the current file's percent bar", () => {
  // Regression test: message and percent must render together, not
  // overwrite each other - this is the exact shape a multi-file batch
  // produces (a Message naming the file, then Percent updates for it).
  render(
    <RunningView toolTitle="Convert video" message="Converting 2 of 3: clip-2.mp4" percent={40} />,
  );
  expect(screen.getByText("Converting 2 of 3: clip-2.mp4")).toBeInTheDocument();
  expect(screen.getByTestId("progress-fill")).toHaveStyle({ width: "40%" });
});
