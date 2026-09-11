import { render, screen } from "@testing-library/react";
import { test, expect } from "vitest";
import { SystemStatusSection } from "./SystemStatusSection";
import type { CapabilitiesReport } from "../lib/wield";

const report: CapabilitiesReport = {
  binaries: { magick: true, ffmpeg: false },
  portals: { Screenshot: 2 },
  tools: [
    { id: "color.pick", title: "Pick a colour", available: true, reason: null },
    { id: "image.convert", title: "Convert image", available: false, reason: "magick is not installed" },
  ],
};

test("shows a loading state before the report arrives", () => {
  render(<SystemStatusSection report={null} />);
  expect(screen.getByText(/checking/i)).toBeInTheDocument();
});

test("lists binaries, portals, and per-tool availability", () => {
  render(<SystemStatusSection report={report} />);
  expect(screen.getByText("magick")).toBeInTheDocument();
  expect(screen.getByText("Found")).toBeInTheDocument();
  expect(screen.getByText("ffmpeg")).toBeInTheDocument();
  expect(screen.getByText("Not found")).toBeInTheDocument();
  expect(screen.getByText("Screenshot")).toBeInTheDocument();
  expect(screen.getByText("v2")).toBeInTheDocument();
  expect(screen.getByText("magick is not installed")).toBeInTheDocument();
});

test("shows a message when no portals are detected", () => {
  render(<SystemStatusSection report={{ binaries: {}, portals: {}, tools: [] }} />);
  expect(screen.getByText(/no portals detected/i)).toBeInTheDocument();
});
