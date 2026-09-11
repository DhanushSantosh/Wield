import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { ToolRow } from "./ToolRow";

const tool = {
  id: "image.convert",
  title: "Convert image",
  keywords: [],
  category: "Convert" as const,
  args: [],
  available: true,
  reason: null,
};

test("shows category when available", () => {
  render(
    <ToolRow
      tool={tool}
      selected={false}
      onSelect={vi.fn()}
      onActivate={vi.fn()}
    />,
  );
  expect(screen.getByText("Convert")).toBeInTheDocument();
});

test("shows the reason instead of category when unavailable", () => {
  const unavailable = { ...tool, available: false, reason: "magick is not installed" };
  render(
    <ToolRow
      tool={unavailable}
      selected={false}
      onSelect={vi.fn()}
      onActivate={vi.fn()}
    />,
  );
  expect(screen.getByText("magick is not installed")).toBeInTheDocument();
});

test("an unavailable row cannot be activated", async () => {
  const onActivate = vi.fn();
  render(
    <ToolRow
      tool={{ ...tool, available: false, reason: "missing dependency" }}
      selected
      onSelect={vi.fn()}
      onActivate={onActivate}
    />,
  );
  screen.getByRole("button").click();
  expect(onActivate).not.toHaveBeenCalled();
});
