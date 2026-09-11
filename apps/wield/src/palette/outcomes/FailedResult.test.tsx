import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { FailedResult } from "./FailedResult";

test("renders failure details and exposes retry and copy actions", async () => {
  const onRetry = vi.fn();
  const onCopyDetails = vi.fn();
  render(
    <FailedResult
      stage="Command"
      detail="exit code 1"
      hint="Check the input"
      onRetry={onRetry}
      onCopyDetails={onCopyDetails}
    />,
  );
  expect(screen.getByText("exit code 1")).toBeInTheDocument();
  expect(screen.getByText("Check the input")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Retry" }));
  await userEvent.click(screen.getByRole("button", { name: "Copy details" }));
  expect(onRetry).toHaveBeenCalled();
  expect(onCopyDetails).toHaveBeenCalled();
});
