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
