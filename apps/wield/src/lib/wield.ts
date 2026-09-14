import { Channel, invoke } from "@tauri-apps/api/core";

export type Category = "Capture" | "Convert" | "Desktop";

export type ArgValueLiteral =
  | { Str: string }
  | { Int: number }
  | { Float: number }
  | { Bool: boolean };

export interface FileFilter {
  label: string;
  extensions: string[];
}

export type ArgType =
  | { File: { filters: FileFilter[]; multiple: boolean } }
  | "Dir"
  | "Str"
  | { Int: { range: [number, number] | null; step: number | null } }
  | { Float: { range: [number, number] | null } }
  | "Bool"
  | { Enum: { options: string[] } }
  | "Text";

export interface When {
  arg: string;
  in: ArgValueLiteral[];
}

export interface ArgSpec {
  name: string;
  label: string;
  help: string | null;
  arg_type: ArgType;
  default: ArgValueLiteral | null;
  required: boolean;
  when: When | null;
}

export interface ToolSummary {
  id: string;
  title: string;
  keywords: string[];
  category: Category;
  args: ArgSpec[];
  available: boolean;
  reason: string | null;
}

export type RunId = string;

export type Progress =
  | "Started"
  | { Percent: number }
  | { Message: string }
  | "Finished";

export type Stage = "Validation" | "Portal" | "Command" | "Output" | "Native";
export type ValueKind = "Color" | "Text";

export type ToolOutcome =
  | { Value: { kind: ValueKind; data: string } }
  | { File: { path: string } }
  | { Report: { title: string; lines: string[] } }
  | { Unavailable: { reason: string; fix: string | null } }
  | { Failed: { stage: Stage; detail: string; hint: string | null } }
  | "Cancelled";

export interface RunResult {
  run_id: RunId;
  outcome: ToolOutcome;
}

export function createRunId(): RunId {
  return crypto.randomUUID();
}

export async function listTools(query?: string): Promise<ToolSummary[]> {
  return invoke<ToolSummary[]>("list_tools", { query: query ?? null });
}

export async function runTool(
  id: string,
  args: Record<string, unknown>,
  onProgress: (progress: Progress) => void,
  runId: RunId,
): Promise<RunResult> {
  const progress = new Channel<Progress>();
  progress.onmessage = onProgress;
  return invoke<RunResult>("run_tool", { id, args, runId, progress });
}

export async function cancelRun(runId: RunId): Promise<boolean> {
  return invoke<boolean>("cancel", { runId });
}

export async function hidePalette(): Promise<void> {
  return invoke<void>("hide_palette");
}

/**
 * Re-shows the palette after it was hidden for something other than the
 * user dismissing it - see FormField's file/folder pickers, which hide it
 * first to release its KeyboardMode::Exclusive grab (see layer_shell.rs)
 * so a native file-chooser dialog can actually receive input, then bring
 * it back afterward.
 */
export async function showPalette(): Promise<void> {
  return invoke<void>("show_palette");
}

// HotkeyState is internally tagged (#[serde(tag = "state")]) on the Rust
// side — unlike every other enum here, which is externally tagged.
export type HotkeyState =
  | { state: "Pending" }
  | { state: "Registered" }
  | { state: "Unavailable"; fallback_command: string };

export interface ToolAvailability {
  id: string;
  title: string;
  available: boolean;
  reason: string | null;
}

export interface CapabilitiesReport {
  binaries: Record<string, boolean>;
  portals: Record<string, number>;
  tools: ToolAvailability[];
}

export async function hotkeyStatus(): Promise<HotkeyState> {
  return invoke<HotkeyState>("hotkey_status");
}

/**
 * Opens the desktop environment's own shortcut-rebinding UI. Rejects when
 * there's no active portal session to reconfigure (the bind never
 * succeeded, hasn't resolved yet, or the backend doesn't support it).
 */
export async function configureHotkey(): Promise<void> {
  return invoke<void>("configure_hotkey");
}

export async function capabilities(): Promise<CapabilitiesReport> {
  return invoke<CapabilitiesReport>("capabilities");
}

/** Subscribes to the tray's tool-selection event; returns an unsubscribe function. */
export async function onSelectTool(handler: (toolId: string) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("tray://select-tool", (event) => handler(event.payload));
}

/**
 * Subscribes to the tray's "Preferences" selection. Settings lives inside
 * the palette window as its own view (see App.tsx) rather than a separate
 * window, so opening it from the tray needs an event the same way
 * tool-selection does, not a second window to show.
 */
export async function onOpenSettings(handler: () => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<void>("tray://open-settings", () => handler());
}
