import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { SearchView } from "./SearchView";

const tools = [
  {
    id: "a",
    title: "A tool",
    keywords: [],
    category: "Convert" as const,
    args: [],
    available: true,
    reason: null,
  },
];

const baseProps = {
  query: "",
  onQueryChange: vi.fn(),
  tools,
  showingRecents: true,
  selectedIndex: 0,
  onSelectIndex: vi.fn(),
  onActivate: vi.fn(),
  onOpenSettings: vi.fn(),
};

test("shows the Recent label only when showingRecents is true", () => {
  const { rerender } = render(<SearchView {...baseProps} />);
  expect(screen.getByText("Recent")).toBeInTheDocument();
  rerender(<SearchView {...baseProps} query="a" showingRecents={false} />);
  expect(screen.queryByText("Recent")).not.toBeInTheDocument();
});

test("typing calls onQueryChange", async () => {
  const onQueryChange = vi.fn();
  render(<SearchView {...baseProps} tools={[]} onQueryChange={onQueryChange} />);
  await userEvent.type(screen.getByPlaceholderText("Search tools…"), "x");
  expect(onQueryChange).toHaveBeenCalledWith("x");
});

test("the settings button calls onOpenSettings", async () => {
  const onOpenSettings = vi.fn();
  render(<SearchView {...baseProps} onOpenSettings={onOpenSettings} />);
  await userEvent.click(screen.getByRole("button", { name: "Open settings" }));
  expect(onOpenSettings).toHaveBeenCalledTimes(1);
});
