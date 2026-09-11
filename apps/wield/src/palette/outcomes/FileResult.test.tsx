import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { FileResult } from "./FileResult";

test("renders the path and exposes both file actions", async () => {
  const onOpenFolder = vi.fn();
  const onRunAgain = vi.fn();
  render(
    <FileResult path="/tmp/result.png" onOpenFolder={onOpenFolder} onRunAgain={onRunAgain} />,
  );
  expect(screen.getByText("/tmp/result.png")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Open folder" }));
  await userEvent.click(screen.getByRole("button", { name: "Run again" }));
  expect(onOpenFolder).toHaveBeenCalled();
  expect(onRunAgain).toHaveBeenCalled();
});
