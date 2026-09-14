import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import type { ArgSpec, ToolSummary } from "../lib/wield";
import { ArgForm } from "./ArgForm";
import { isVisible } from "./argVisibility";

const widthSpec: ArgSpec = {
  name: "width",
  label: "Width",
  help: null,
  arg_type: { Int: { range: null, step: null } },
  default: null,
  required: false,
  when: { arg: "format", in: [] },
};

test("isVisible: presence form is satisfied by any value on the referenced arg", () => {
  expect(isVisible(widthSpec, {}, [widthSpec])).toBe(false);
  expect(isVisible(widthSpec, { format: "webp" }, [widthSpec])).toBe(true);
});

test("renders only visible fields and submits their default values", async () => {
  const tool: ToolSummary = {
    id: "image.convert",
    title: "Convert image",
    keywords: [],
    category: "Convert",
    available: true,
    reason: null,
    args: [
      {
        name: "format",
        label: "Output format",
        help: null,
        arg_type: { Enum: { options: ["png", "webp"] } },
        default: { Str: "png" },
        required: true,
        when: null,
      },
    ],
  };
  const onSubmit = vi.fn();
  render(<ArgForm tool={tool} initialValues={{}} onSubmit={onSubmit} onEscape={vi.fn()} />);
  await userEvent.click(screen.getByRole("button", { name: "Convert image" }));
  expect(onSubmit).toHaveBeenCalledWith({ format: "png" });
});

test("initialValues pre-fills the form for Run again", () => {
  const tool: ToolSummary = {
    id: "x",
    title: "X",
    keywords: [],
    category: "Convert",
    available: true,
    reason: null,
    args: [
      {
        name: "note",
        label: "Note",
        help: null,
        arg_type: "Str",
        default: null,
        required: false,
        when: null,
      },
    ],
  };
  render(
    <ArgForm
      tool={tool}
      initialValues={{ note: "hello" }}
      onSubmit={vi.fn()}
      onEscape={vi.fn()}
    />,
  );
  expect(screen.getByDisplayValue("hello")).toBeInTheDocument();
});

test("hiding a field via its when-gate excludes its value from submit, even if it was set while visible", async () => {
  const tool: ToolSummary = {
    id: "audio.extract",
    title: "Extract audio",
    keywords: [],
    category: "Convert",
    available: true,
    reason: null,
    args: [
      {
        name: "format",
        label: "Output format",
        help: null,
        arg_type: { Enum: { options: ["mp3", "flac"] } },
        default: { Str: "mp3" },
        required: true,
        when: null,
      },
      {
        name: "quality",
        label: "Bitrate",
        help: null,
        arg_type: { Enum: { options: ["192k", "320k"] } },
        default: null,
        required: false,
        when: { arg: "format", in: [{ Str: "mp3" }] },
      },
    ],
  };
  const onSubmit = vi.fn();
  render(<ArgForm tool={tool} initialValues={{}} onSubmit={onSubmit} onEscape={vi.fn()} />);
  // quality is visible while format=mp3 (the default) - set it.
  await userEvent.selectOptions(screen.getByLabelText("Bitrate"), "320k");
  // Switching format to flac hides quality's field. Its value stays in
  // React state (ArgForm never clears it on a visibility change) - this
  // is exactly the scenario the fix must handle at submit time.
  await userEvent.selectOptions(screen.getByLabelText("Output format"), "flac");
  expect(screen.queryByLabelText("Bitrate")).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Extract audio" }));
  expect(onSubmit).toHaveBeenCalledWith({ format: "flac" });
});
