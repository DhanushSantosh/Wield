import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

const openMock = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openMock(...args),
}));

import type { ArgSpec } from "../lib/wield";
import { FormField } from "./FormField";

function spec(argType: ArgSpec["arg_type"]): ArgSpec {
  return {
    name: "value",
    label: "Value",
    help: null,
    arg_type: argType,
    default: null,
    required: false,
    when: null,
  };
}

test("File opens the picker and returns the selected path", async () => {
  openMock.mockResolvedValueOnce("/tmp/photo.png");
  const onChange = vi.fn();
  render(
    <FormField
      spec={spec({ File: { filters: [{ label: "Images", extensions: ["png"] }], multiple: false } })}
      value={undefined}
      onChange={onChange}
    />,
  );
  await userEvent.click(screen.getByRole("button", { name: "Choose a file…" }));
  expect(openMock).toHaveBeenCalledWith({
    multiple: false,
    filters: [{ name: "Images", extensions: ["png"] }],
  });
  expect(onChange).toHaveBeenCalledWith("/tmp/photo.png");
});

test("Dir opens a folder picker and returns the selected path", async () => {
  openMock.mockResolvedValueOnce("/tmp/output");
  const onChange = vi.fn();
  render(<FormField spec={spec("Dir")} value={undefined} onChange={onChange} />);
  await userEvent.click(screen.getByRole("button", { name: "Choose a folder…" }));
  expect(openMock).toHaveBeenCalledWith({ directory: true });
  expect(onChange).toHaveBeenCalledWith("/tmp/output");
});

test("Str returns text input changes", () => {
  const onChange = vi.fn();
  render(<FormField spec={spec("Str")} value="old" onChange={onChange} />);
  const input = screen.getByRole("textbox");
  fireEvent.change(input, { target: { value: "new" } });
  expect(onChange).toHaveBeenLastCalledWith("new");
});

test("Text uses a multiline control", () => {
  const onChange = vi.fn();
  render(<FormField spec={spec("Text")} value="notes" onChange={onChange} />);
  const input = screen.getByRole("textbox");
  expect(input.tagName).toBe("TEXTAREA");
  fireEvent.change(input, { target: { value: "more notes" } });
  expect(onChange).toHaveBeenCalledWith("more notes");
});

test("Int configures its range and returns a number", () => {
  const onChange = vi.fn();
  render(
    <FormField
      spec={spec({ Int: { range: [1, 10], step: 2 } })}
      value={3}
      onChange={onChange}
    />,
  );
  const input = screen.getByRole("spinbutton");
  expect(input).toHaveAttribute("min", "1");
  expect(input).toHaveAttribute("max", "10");
  expect(input).toHaveAttribute("step", "2");
  fireEvent.change(input, { target: { value: "7" } });
  expect(onChange).toHaveBeenCalledWith(7);
});

test("Float accepts decimal values with an optional range", () => {
  const onChange = vi.fn();
  render(
    <FormField
      spec={spec({ Float: { range: [0, 1] } })}
      value={0.25}
      onChange={onChange}
    />,
  );
  const input = screen.getByRole("spinbutton");
  expect(input).toHaveAttribute("step", "any");
  fireEvent.change(input, { target: { value: "0.75" } });
  expect(onChange).toHaveBeenCalledWith(0.75);
});

test("Bool returns the checkbox state", async () => {
  const onChange = vi.fn();
  render(<FormField spec={spec("Bool")} value={false} onChange={onChange} />);
  await userEvent.click(screen.getByRole("checkbox"));
  expect(onChange).toHaveBeenCalledWith(true);
});

test("Enum renders every option and returns the selection", async () => {
  const onChange = vi.fn();
  render(
    <FormField
      spec={spec({ Enum: { options: ["png", "webp"] } })}
      value="png"
      onChange={onChange}
    />,
  );
  await userEvent.selectOptions(screen.getByRole("combobox"), "webp");
  expect(onChange).toHaveBeenCalledWith("webp");
  expect(screen.getAllByRole("option")).toHaveLength(2);
});
