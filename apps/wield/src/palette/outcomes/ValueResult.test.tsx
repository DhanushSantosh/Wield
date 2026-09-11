import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { ValueResult } from "./ValueResult";

test("renders color representations and copies the primary value", async () => {
  const onCopy = vi.fn();
  render(
    <ValueResult
      kind="Color"
      data={'{"hex":"#3ed0c4","rgb":"rgb(62, 208, 196)","hsl":"hsl(175, 61%, 53%)"}'}
      onCopy={onCopy}
    />,
  );
  expect(screen.getByText("#3ed0c4")).toBeInTheDocument();
  expect(screen.getByText("rgb(62, 208, 196)")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Copy value" }));
  expect(onCopy).toHaveBeenCalled();
});
