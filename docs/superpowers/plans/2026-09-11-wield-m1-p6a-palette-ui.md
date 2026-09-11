# Wield M1 · Plan P6a — Palette UI — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `apps/wield/src`'s P1 placeholder with the real palette UI: search with recents, keyboard navigation, a generated argument form, a running/progress view, and result cards for every `ToolOutcome` variant — plus the two small additive backend changes it needs.

**Status:** Complete on `feat/p6a-palette-ui` — 2026-09-11. The live-cancel contract received one necessary correction: the frontend supplies the run ID before invoking `run_tool`, while backend/D-Bus callers retain generated-ID fallback behavior. Full format, clippy, Rust, TypeScript, lint, and frontend test gates are green. The native app launched, owned its D-Bus name, accepted `ShowPalette`, and completed a real PNG→WebP run; browser preview confirmed the visual shell. Native-window UI automation was unavailable, so the complete interaction matrix is verified by the 10 App integration tests rather than claimed as a hand-driven native-window pass.

**GEON review — 2026-09-11:** the `run_id`-before-invoke gap SUNIO caught (Task 2) was a real defect in this plan, not scope creep — the plan as originally written had no way for the frontend to cancel a run in progress, contradicting §4's "Escape while running cancels." The fix (`requested_run_id: Option<RunId>` with backend-default fallback) is the correct minimal shape; reviewed against `commands.rs` directly, confirmed sound. Independently re-ran `cargo fmt --check` / `clippy -D warnings` / `cargo test --workspace` / `npm run check` (51 frontend tests, 15 files) — all green. Spot-checked `lib/wield.ts`'s TS types against the real Rust serde output, `argVisibility.ts`'s presence-form + reference-chaining logic against `wield-core`'s rule, and `styles.css` against the locked design tokens — all faithful. Rebuilt the real UI bundle (`npm run build:ui`) and relaunched the native binary: headless, owns the D-Bus name, tray builds, portal binds — no regressions from P5b. Attempted one live screenshot via an interactive desktop session; the bound `Super+W` shortcut did not surface the window (likely a window-manager-level conflict, not a Wield defect — see `docs/testing.md`) — not chased further, doesn't block this review given the strength of the automated coverage.

**Architecture:** `App.tsx` holds a small `useReducer` state machine (`search | form | running | result`) and renders one of `SearchView` / `ArgForm` / `RunningView` / `ResultView` accordingly. All backend access goes through `lib/wield.ts`, a typed wrapper over `invoke` whose TypeScript types mirror `wield-core`'s serde output exactly (externally-tagged enums; unit variants as bare strings). `list_tools` gains an optional search query (reusing `Registry::search`) and a per-tool unavailability `reason`; `run_tool` gains a `Channel<Progress>` parameter so progress that P5a discarded now reaches the UI live.

**Tech Stack:** React 19, TypeScript, Vite, Vitest + Testing Library (all already set up). New: `@tauri-apps/plugin-dialog` (file/dir pickers — portal-backed on Linux) and `@tauri-apps/plugin-opener` ("open folder").

**Spec:** `docs/superpowers/specs/2026-09-11-wield-p6a-palette-design.md` — this plan implements it section by section. That spec's §2 (visual design system) and §3 (state catalogue) are the source of truth for every colour/spacing value below; this plan doesn't repeat the rationale, only the values needed to implement.

---

## GEON amendment — 2026-09-11

- **Scope:** `apps/wield/src` (frontend) + the two named additive changes to `apps/wield/src-tauri/src/commands.rs`. No other backend crate changes. No tray/Preferences/System Status (P6b). No blur-to-hide *toggle* (the call is always on in P6a; a Preferences setting to disable it is P6b — the code path is identical either way).
- **Branch:** `feat/p6a-palette-ui` off `master`, per-task commits, push at the end, do **not** open the PR.
- **`cargo` is not on the default `PATH`.** `export PATH="$HOME/.cargo/bin:$PATH"` before every cargo invocation.
- **Serde shapes are load-bearing.** `wield-core`'s enums are `#[derive(Serialize)]` with **no** `#[serde(tag = ...)]`, i.e. default externally-tagged: a unit variant (`Cancelled`, `Started`, `Str` as an `ArgType`, every `Stage`/`Category`/`ValueKind` value) serializes as a **bare JSON string**; a struct-variant (`File { path }`) as `{"File": {"path": "..."}}`; `RunId` is a 1-tuple newtype and serializes as a **bare string**, not `{"RunId": "..."}`. The TS types in Task 4 encode this exactly — do not "simplify" to a `{ type, ...fields }` shape without also changing the Rust side; there is no reason to in this plan.
- **Mechanical-correction rule (as every prior plan):** exact `@tauri-apps/plugin-*` API names/versions may drift — check what's actually published/resolves and adapt; note it in the Task 14 report. Stop-and-ask only for a genuine design fork.
- **`npm run check`** green at the end of every task that touches buildable code.

---

## Global Constraints

- **Repo root:** `/home/dhanush/Projects/Wield` (branch `feat/p6a-palette-ui` off `master`).
- **No inline styles for anything covered by a design token** (§2 of the spec) — use the CSS custom properties from Task 6 everywhere a color/spacing value from that section applies. One-off layout tweaks (flex gaps, etc.) that aren't design-system values are fine as plain CSS.
- **No new state-management library.** `useReducer` in `App.tsx` per the design spec §5 — the state machine is small and fully enumerable.
- **Every `invoke` call goes through `lib/wield.ts`.** No component calls `@tauri-apps/api/core`'s `invoke` directly — this is what makes every component testable by mocking one module.
- **`npm run check`** green at the end of every task that touches buildable code.

---

## File Structure

**Modified — `apps/wield/src-tauri/src/`:**

| File | Change |
| --- | --- |
| `commands.rs` | `ToolSummary` gains `reason: Option<String>`; `list_tools_impl`/`list_tools` gain `query: Option<String>` and populate `reason`; `run_tool`/`run_tool_impl` gain progress forwarding |
| `Cargo.toml` | add `tauri-plugin-dialog`, `tauri-plugin-opener` |
| `lib.rs` | register the two new plugins (`tauri::Builder::default().plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_opener::init())`) |
| `tauri.conf.json` / `capabilities/default.json` | grant the `palette` window the dialog/opener permission identifiers |

**Modified — `apps/wield/`:**

| File | Change |
| --- | --- |
| `package.json` | add `@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-opener` |
| `index.html` | link the stylesheet (Task 6) if not auto-imported via `main.tsx` |

**Replaced — `apps/wield/src/`:**

| File | Responsibility |
| --- | --- |
| `App.tsx` | state machine + top-level layout, replaces the P1 placeholder |
| `App.test.tsx` | replaced with the Task 13 integration tests |

**Created — `apps/wield/src/`:**

| File | Responsibility |
| --- | --- |
| `styles.css` | design tokens (custom properties) + base element styles, imported once in `main.tsx` |
| `lib/wield.ts` | TS mirrors of the Rust serde types + typed `invoke` wrappers (`listTools`, `runTool`, `cancel`) |
| `lib/recents.ts` | localStorage-backed recent-tool-id list |
| `useKeyboardNav.ts` | arrow/enter/escape hook shared by the list views |
| `palette/ToolRow.tsx` | one result row |
| `palette/SearchView.tsx` | query input + recents/results list |
| `palette/FormField.tsx` | one control per `ArgType` |
| `palette/ArgForm.tsx` | the generated argument form |
| `palette/RunningView.tsx` | spinner + progress bar |
| `palette/ResultView.tsx` | dispatches on `ToolOutcome` tag |
| `palette/outcomes/ValueResult.tsx` | `Value` outcome card |
| `palette/outcomes/FileResult.tsx` | `File` outcome card |
| `palette/outcomes/ReportResult.tsx` | `Report` outcome card |
| `palette/outcomes/UnavailableResult.tsx` | `Unavailable` outcome card |
| `palette/outcomes/FailedResult.tsx` | `Failed` outcome card |

**Test files** sit beside each source file (`Foo.tsx` → `Foo.test.tsx`), Vitest convention already used by P1.

**Out of P6a scope:** Preferences window content, System Status, tray-menu-driven tool selection wiring (P5b already emits `tray://select-tool`; *listening* for it is P6b, since it needs the palette to already exist — a one-line addition when P6b lands, not blocked by anything here), first-run onboarding.

---

## Task 1: Backend — `list_tools` search query + unavailable reason

**Files:**
- Modify: `apps/wield/src-tauri/src/commands.rs`

**Interfaces:**
- `ToolSummary` gains `pub reason: Option<String>` (after `available`).
- `pub fn list_tools_impl(state: &AppState, query: Option<&str>) -> Vec<ToolSummary>`:
  ```rust
  pub fn list_tools_impl(state: &AppState, query: Option<&str>) -> Vec<ToolSummary> {
      let descriptors: Vec<&wield_core::Descriptor> = match query.map(str::trim).filter(|q| !q.is_empty()) {
          Some(q) => state.registry.search(q),
          None => state.registry.list().iter().collect(),
      };
      descriptors
          .into_iter()
          .map(|descriptor| {
              let (available, reason) =
                  crate::capabilities::is_available(&state.availability, &descriptor.requires);
              ToolSummary {
                  id: descriptor.id.as_ref().to_owned(),
                  title: descriptor.title.clone(),
                  keywords: descriptor.keywords.clone(),
                  category: match descriptor.category {
                      wield_core::Category::Capture => "Capture",
                      wield_core::Category::Convert => "Convert",
                      wield_core::Category::Desktop => "Desktop",
                  }
                  .to_owned(),
                  args: descriptor.args.clone(),
                  available,
                  reason,
              }
          })
          .collect()
  }

  #[tauri::command]
  pub fn list_tools(state: tauri::State<'_, AppState>, query: Option<String>) -> Vec<ToolSummary> {
      list_tools_impl(&state, query.as_deref())
  }
  ```

- [x] **Step 1: Write the failing tests**

Append to the `#[cfg(test)] mod tests` block in `commands.rs`:

```rust
#[test]
fn list_tools_with_no_query_returns_registration_order() {
    let state = AppState::for_test_sync(); // see note below
    let all = list_tools_impl(&state, None);
    assert_eq!(all.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), vec!["color.pick", "image.convert"]);
}

#[test]
fn list_tools_with_a_query_ranks_matches() {
    let state = AppState::for_test_sync();
    let hits = list_tools_impl(&state, Some("img conv"));
    assert_eq!(hits.first().unwrap().id, "image.convert");
    assert!(list_tools_impl(&state, Some("zzz nonsense")).is_empty());
}

#[test]
fn unavailable_tool_carries_a_reason() {
    let state = AppState::for_test_sync(); // built with an empty BinaryResolver so magick resolves to nothing
    let convert = list_tools_impl(&state, None)
        .into_iter()
        .find(|t| t.id == "image.convert")
        .unwrap();
    assert!(!convert.available);
    assert!(convert.reason.unwrap().contains("magick"));
}
```
`AppState::for_test_sync()` doesn't exist yet — add a `#[cfg(test)]` sync wrapper in `state.rs` next to the existing async `for_test`:
```rust
#[cfg(test)]
pub(crate) fn for_test_sync() -> Self {
    tauri::async_runtime::block_on(Self::for_test(
        wield_tools::builtin_registry(),
        wield_core::BinaryResolver::with_dirs(vec![]), // nothing resolves -> image.convert unavailable
    ))
}
```
(If `tauri::async_runtime::block_on` isn't available/appropriate outside a running app in this Tauri version, use `tokio::runtime::Runtime::new().unwrap().block_on(...)` instead — mechanical correction, note it.)

- [x] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-app`
Expected: FAIL (new fields/params don't exist).

- [x] **Step 3: Implement**

- [x] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-app`
Expected: PASS — all prior P5a/P5b tests plus the three new ones. `tray.rs`'s `ToolSummary` fixtures (P5b) now need `reason: None` added — fix those construction sites too.

- [x] **Step 5: Commit**

```bash
git checkout -b feat/p6a-palette-ui
git add apps/wield/src-tauri
git commit -m "feat(app): list_tools search query + unavailable reason"
```

---

## Task 2: Backend — `run_tool` progress channel

**Files:**
- Modify: `apps/wield/src-tauri/src/commands.rs`

**Interfaces:**

```rust
pub async fn run_tool_impl(
    state: &AppState,
    id: &str,
    args: &serde_json::Value,
    mut on_progress: impl FnMut(wield_core::Progress) + Send + 'static,
) -> Result<RunResult, String> {
    let descriptor = state.registry.get(id).cloned().ok_or_else(|| format!("unknown tool: {id}"))?;
    let args = coerce_json_args(&descriptor, args)?;
    let run_id = crate::state::RunId::new();
    let token = CancellationToken::new();
    state.register_run(run_id.clone(), token.clone());

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(32);
    let forward = tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            on_progress(progress);
        }
    });

    let started = std::time::Instant::now();
    let outcome = state
        .executor
        .run(ExecutionRequest { descriptor, args }, progress_tx, token)
        .await;
    let _ = forward.await;
    state.take_run(&run_id);
    tracing::info!(tool_id = id, elapsed_ms = started.elapsed().as_millis() as u64, outcome = ?outcome, "tool run finished");
    Ok(RunResult { run_id, outcome })
}

#[tauri::command]
pub async fn run_tool(
    state: tauri::State<'_, AppState>,
    id: String,
    args: serde_json::Value,
    progress: tauri::ipc::Channel<wield_core::Progress>,
) -> Result<RunResult, String> {
    run_tool_impl(&state, &id, &args, move |p| {
        let _ = progress.send(p);
    })
    .await
}
```
Every existing call site of `run_tool_impl` in `commands.rs`'s tests (P5a's `run_tool_runs_a_command_tool_against_a_stub`, `run_tool_rejects_unknown_arguments` — wait, that one calls `coerce_json_args` directly, unaffected — and `cancel_stops_an_in_flight_run`) must add a no-op `|_p| {}` (or a `Vec`-collecting closure where a test wants to assert on progress) as the fourth argument. `PendingShell::run_tool` in `lib.rs` (the D-Bus bridge, P5a) also calls `run_tool_impl` — update it to pass `|_p| {}` too (the D-Bus one-shot path doesn't stream progress; that's fine, it's a different consumer).

- [x] **Step 1: Write the failing test**

Append to `commands.rs`'s tests:

```rust
#[tokio::test]
async fn run_tool_forwards_progress_before_the_final_outcome() {
    let directory = tempfile::tempdir().unwrap();
    write_stub(directory.path(), "#!/bin/sh\nlast=\"\"\nfor arg in \"$@\"; do last=\"$arg\"; done\ncat \"$1\" > \"$last\"\n");
    let input = directory.path().join("in.png");
    std::fs::write(&input, b"IMG").unwrap();
    let state = AppState::for_test(
        wield_tools::builtin_registry(),
        wield_core::BinaryResolver::with_dirs(vec![directory.path().to_path_buf()]),
    ).await;
    let args = serde_json::json!({ "input": input.to_string_lossy(), "format": "png" });

    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_clone = seen.clone();
    let result = run_tool_impl(&state, "image.convert", &args, move |p| {
        seen_clone.lock().unwrap().push(p);
    }).await.unwrap();

    assert!(matches!(result.outcome, ToolOutcome::File { .. }));
    let events = seen.lock().unwrap();
    assert!(events.contains(&wield_core::Progress::Started));
    assert!(events.contains(&wield_core::Progress::Finished));
}
```
(`wield_core::Progress` needs `PartialEq` to support `contains` — check `outcome.rs`; it already derives `PartialEq`, confirmed in P2.)

- [x] **Step 2: Run to verify failure** — `cargo test -p wield-app run_tool_forwards_progress`
- [x] **Step 3: Implement; update every existing `run_tool_impl` call site (tests + `PendingShell`)**
- [x] **Step 4: Run to verify pass** — `cargo test -p wield-app`
- [x] **Step 5: Commit**

```bash
git add apps/wield/src-tauri
git commit -m "feat(app): run_tool streams Progress over a Tauri channel"
```

---

## Task 3: Frontend — dialog + opener plugins

**Files:**
- Modify: `apps/wield/package.json`, `apps/wield/src-tauri/Cargo.toml`, `apps/wield/src-tauri/src/lib.rs`, `apps/wield/src-tauri/capabilities/default.json`

**Interfaces:**
- `package.json` dependencies: `"@tauri-apps/plugin-dialog": "^2"`, `"@tauri-apps/plugin-opener": "^2"`.
- `src-tauri/Cargo.toml` `[dependencies]`: `tauri-plugin-dialog = "2"`, `tauri-plugin-opener = "2"`.
- `lib.rs`: in the `tauri::Builder::default()` chain, add `.plugin(tauri_plugin_dialog::init())` and `.plugin(tauri_plugin_opener::init())` before `.manage(state)` (plugin registration order relative to `.manage` doesn't matter, but do it before `.build(...)`).
- `capabilities/default.json`: add the dialog and opener default permission identifiers to `"permissions"` (e.g. `"dialog:default"`, `"opener:default"` — confirm the exact identifiers each plugin's own capability schema expects once installed; they publish a `default` permission set, use it rather than hand-picking individual commands).

- [x] **Step 1: Install and register**

```bash
cd /home/dhanush/Projects/Wield
npm install @tauri-apps/plugin-dialog@^2 @tauri-apps/plugin-opener@^2 -w apps/wield
export PATH="$HOME/.cargo/bin:$PATH"
cd apps/wield/src-tauri && cargo add tauri-plugin-dialog@2 tauri-plugin-opener@2 && cd -
```

- [x] **Step 2: Wire `lib.rs` and `capabilities/default.json`**

- [x] **Step 3: Verify the backend still builds**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && rm -rf apps/wield/src-tauri/gen && cargo check -p wield-app`
Expected: PASS.

- [x] **Step 4: `npm run check`** — PASS (no frontend code uses the new packages yet; this just confirms install + typecheck don't choke on them).

- [x] **Step 5: Commit**

```bash
git add apps/wield/package.json package-lock.json apps/wield/src-tauri
git commit -m "chore(app): add dialog and opener Tauri plugins"
```

---

## Task 4: `lib/wield.ts` — typed backend access

**Files:**
- Create: `apps/wield/src/lib/wield.ts`, `apps/wield/src/lib/wield.test.ts`

**Interfaces:**

```typescript
// Mirrors wield-core's serde output exactly. Unit variants are bare strings;
// struct variants are {"Variant": {...}}; RunId is a bare string (newtype).

export type Category = "Capture" | "Convert" | "Desktop";

export type ArgValueLiteral =
  | { Str: string }
  | { Int: number }
  | { Float: number }
  | { Bool: boolean };

export interface FileFilter { label: string; extensions: string[] }

export type ArgType =
  | { File: { filters: FileFilter[]; multiple: boolean } }
  | "Dir"
  | "Str"
  | { Int: { range: [number, number] | null; step: number | null } }
  | { Float: { range: [number, number] | null } }
  | "Bool"
  | { Enum: { options: string[] } }
  | "Text";

export interface When { arg: string; in: ArgValueLiteral[] }

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

export type Progress = "Started" | { Percent: number } | { Message: string } | "Finished";

export type Stage = "Validation" | "Portal" | "Command" | "Output" | "Native";
export type ValueKind = "Color" | "Text";

export type ToolOutcome =
  | { Value: { kind: ValueKind; data: string } }
  | { File: { path: string } }
  | { Report: { title: string; lines: string[] } }
  | { Unavailable: { reason: string; fix: string | null } }
  | { Failed: { stage: Stage; detail: string; hint: string | null } }
  | "Cancelled";

export interface RunResult { run_id: RunId; outcome: ToolOutcome }

export async function listTools(query?: string): Promise<ToolSummary[]>;
export async function runTool(
  id: string,
  args: Record<string, unknown>,
  onProgress: (p: Progress) => void,
): Promise<RunResult>;
export async function cancelRun(runId: RunId): Promise<boolean>;
```

Implementation notes:
- `listTools` → `invoke<ToolSummary[]>("list_tools", { query: query ?? null })`.
- `runTool` creates a `new Channel<Progress>()` from `@tauri-apps/api/core`, sets `channel.onmessage = onProgress`, and calls `invoke<RunResult>("run_tool", { id, args, progress: channel })`.
- `cancelRun` → `invoke<boolean>("cancel", { runId })`.
- A small `ArgValue` encoder is **not** needed here — `args` is passed as a plain JS object (`{ input: "/x/a.png", width: 800 }`) matching the backend's `coerce_json_args`, which already expects bare JSON values keyed by arg name (see P5a `commands.rs`), not the `ArgValueLiteral`-tagged shape (that shape is only for descriptor `default`/`when` values).

- [x] **Step 1: Write the failing tests**

`apps/wield/src/lib/wield.test.ts`:

```typescript
import { describe, expect, test, vi } from "vitest";

const invokeMock = vi.fn();
const channelInstances: any[] = [];
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  Channel: class {
    onmessage: ((payload: unknown) => void) | undefined;
    constructor() { channelInstances.push(this); }
  },
}));

import { listTools, runTool, cancelRun } from "./wield";

test("listTools passes the query through, defaulting to null", async () => {
  invokeMock.mockResolvedValueOnce([]);
  await listTools();
  expect(invokeMock).toHaveBeenCalledWith("list_tools", { query: null });

  invokeMock.mockResolvedValueOnce([]);
  await listTools("convert");
  expect(invokeMock).toHaveBeenCalledWith("list_tools", { query: "convert" });
});

test("runTool wires a Channel and forwards progress events to onProgress", async () => {
  invokeMock.mockImplementationOnce(async () => ({ run_id: "abc", outcome: "Cancelled" }));
  const seen: unknown[] = [];
  const result = await runTool("image.convert", { input: "/x/a.png" }, (p) => seen.push(p));
  expect(result.outcome).toBe("Cancelled");
  const [, payload] = invokeMock.mock.calls[0];
  expect(payload.id).toBe("image.convert");
  expect(payload.args).toEqual({ input: "/x/a.png" });
  // simulate the backend pushing a progress event on the channel
  channelInstances.at(-1)!.onmessage!("Started");
  expect(seen).toEqual(["Started"]);
});

test("cancelRun invokes cancel with the run id", async () => {
  invokeMock.mockResolvedValueOnce(true);
  expect(await cancelRun("abc")).toBe(true);
  expect(invokeMock).toHaveBeenCalledWith("cancel", { runId: "abc" });
});
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement `lib/wield.ts`**
- [x] **Step 4: Run to verify pass** — `npm run test -w apps/wield`
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/lib/wield.ts apps/wield/src/lib/wield.test.ts
git commit -m "feat(app-ui): typed wield-core/Tauri IPC wrapper"
```

---

## Task 5: `lib/recents.ts`

**Files:**
- Create: `apps/wield/src/lib/recents.ts`, `apps/wield/src/lib/recents.test.ts`

**Interfaces:**

```typescript
const KEY = "wield.recents";
const MAX = 8;

export function getRecentIds(): string[]; // most-recent-first, reads localStorage, [] on any error
export function bumpRecent(id: string): void; // moves/inserts id to the front, caps at MAX, swallows storage errors
```

- [x] **Step 1: Write the failing tests**

```typescript
import { beforeEach, expect, test } from "vitest";
import { bumpRecent, getRecentIds } from "./recents";

beforeEach(() => localStorage.clear());

test("bumpRecent inserts new ids at the front", () => {
  bumpRecent("a");
  bumpRecent("b");
  expect(getRecentIds()).toEqual(["b", "a"]);
});

test("bumpRecent moves an existing id to the front instead of duplicating", () => {
  bumpRecent("a");
  bumpRecent("b");
  bumpRecent("a");
  expect(getRecentIds()).toEqual(["a", "b"]);
});

test("bumpRecent caps at 8 entries", () => {
  for (let i = 0; i < 10; i += 1) bumpRecent(`tool-${i}`);
  expect(getRecentIds()).toHaveLength(8);
  expect(getRecentIds()[0]).toBe("tool-9");
});

test("getRecentIds returns an empty list when storage is empty or corrupt", () => {
  expect(getRecentIds()).toEqual([]);
  localStorage.setItem("wield.recents", "not json");
  expect(getRecentIds()).toEqual([]);
});
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/lib/recents.ts apps/wield/src/lib/recents.test.ts
git commit -m "feat(app-ui): localStorage-backed recent tools"
```

---

## Task 6: Design tokens stylesheet

**Files:**
- Create: `apps/wield/src/styles.css`
- Modify: `apps/wield/src/main.tsx` (import it)

**Interfaces:** — verbatim from the design spec §2:

```css
:root {
  --bg: #0a0a0a;
  --surface: #161616;
  --border: #1c1c1c;
  --text: #e8e8e8;
  --text-muted: #6b6b6b;
  --text-faint: #4a4a4a;
  --accent: #3ed0c4;
  --accent-ink: #0c1a18;
  --danger: #d9714f;

  --font-ui: "Inter", -apple-system, "Segoe UI", sans-serif;
  --font-data: ui-monospace, "JetBrains Mono", monospace;

  color-scheme: dark;
}

* { box-sizing: border-box; }

html, body, #root {
  height: 100%;
  margin: 0;
}

body {
  background: transparent; /* the OS window chrome/backdrop provides the frame */
  color: var(--text);
  font-family: var(--font-ui);
  font-size: 14px;
}

.app-shell {
  background: var(--bg);
  border-radius: 15px;
  box-shadow: 0 20px 50px rgba(0, 0, 0, .6), 0 0 0 1px var(--border);
  overflow: hidden;
  max-width: 520px;
  margin: 0 auto;
  transition: max-height 160ms ease;
}

@media (prefers-reduced-motion: reduce) {
  .app-shell { transition: none; }
}

:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}
```
(Additional component-scoped rules are added alongside each component in later tasks — this task only establishes the shared tokens + base reset, matching how `main.tsx` in P1 had no stylesheet at all.)

- [x] **Step 1: Create `styles.css` and import it in `main.tsx`** (`import "./styles.css";` before the `App` import)
- [x] **Step 2: Verify** — `export PATH="$HOME/.cargo/bin:$PATH" && npm run check` → PASS (nothing references the new classes yet, but lint/typecheck/test must stay green).
- [x] **Step 3: Commit**

```bash
git add apps/wield/src/styles.css apps/wield/src/main.tsx
git commit -m "feat(app-ui): design token stylesheet"
```

---

## Task 7: `ToolRow` + `SearchView`

**Files:**
- Create: `apps/wield/src/palette/ToolRow.tsx`, `apps/wield/src/palette/SearchView.tsx`, and their `.test.tsx` files

**Interfaces:**

```typescript
// ToolRow.tsx
export interface ToolRowProps {
  tool: ToolSummary;
  selected: boolean;
  onSelect: () => void; // hover/click sets selection
  onActivate: () => void; // click/Enter runs or opens the form
}
export function ToolRow(props: ToolRowProps): JSX.Element;
```
Renders: an icon placeholder box (`--surface` background, no icon glyphs shipped in P6a — a blank chip is fine, icons are a later polish pass), `tool.title`, and on the right either `tool.category` (available) or `tool.reason` (unavailable, dimmed). `selected` adds the accent tint + left bar per §2/§3. Unavailable rows get `opacity: .45` and are still focusable/selectable (so the reason is discoverable) but `onActivate` no-ops for them.

```typescript
// SearchView.tsx
export interface SearchViewProps {
  query: string;
  onQueryChange: (q: string) => void;
  tools: ToolSummary[]; // already the right list (recents or search results) — SearchView doesn't fetch
  showingRecents: boolean; // controls the "Recent" section label per spec §3.1
  selectedIndex: number;
  onSelectIndex: (i: number) => void;
  onActivate: (tool: ToolSummary) => void;
}
export function SearchView(props: SearchViewProps): JSX.Element;
```
Input row (`→` glyph, `placeholder="Search tools…"`, controlled `value={query}`), a divider, then `showingRecents && <div className="section-label">Recent</div>`, then one `ToolRow` per `tools[i]` with `selected={i === selectedIndex}`.

- [x] **Step 1: Write the failing tests**

`ToolRow.test.tsx`:
```typescript
import { render, screen } from "@testing-library/react";
import { test, expect, vi } from "vitest";
import { ToolRow } from "./ToolRow";

const tool = { id: "image.convert", title: "Convert image", keywords: [], category: "Convert" as const, args: [], available: true, reason: null };

test("shows category when available", () => {
  render(<ToolRow tool={tool} selected={false} onSelect={vi.fn()} onActivate={vi.fn()} />);
  expect(screen.getByText("Convert")).toBeInTheDocument();
});

test("shows the reason instead of category when unavailable", () => {
  const unavailable = { ...tool, available: false, reason: "magick is not installed" };
  render(<ToolRow tool={unavailable} selected={false} onSelect={vi.fn()} onActivate={vi.fn()} />);
  expect(screen.getByText("magick is not installed")).toBeInTheDocument();
});
```

`SearchView.test.tsx`:
```typescript
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { test, expect, vi } from "vitest";
import { SearchView } from "./SearchView";

const tools = [
  { id: "a", title: "A tool", keywords: [], category: "Convert" as const, args: [], available: true, reason: null },
];

test("shows the Recent label only when showingRecents is true", () => {
  const { rerender } = render(<SearchView query="" onQueryChange={vi.fn()} tools={tools} showingRecents onSelectIndex={vi.fn()} selectedIndex={0} onActivate={vi.fn()} />);
  expect(screen.getByText("Recent")).toBeInTheDocument();
  rerender(<SearchView query="a" onQueryChange={vi.fn()} tools={tools} showingRecents={false} onSelectIndex={vi.fn()} selectedIndex={0} onActivate={vi.fn()} />);
  expect(screen.queryByText("Recent")).not.toBeInTheDocument();
});

test("typing calls onQueryChange", async () => {
  const onQueryChange = vi.fn();
  render(<SearchView query="" onQueryChange={onQueryChange} tools={[]} showingRecents onSelectIndex={vi.fn()} selectedIndex={0} onActivate={vi.fn()} />);
  await userEvent.type(screen.getByPlaceholderText("Search tools…"), "x");
  expect(onQueryChange).toHaveBeenCalledWith("x");
});
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement both components + their scoped CSS (append to `styles.css` or a co-located `.css` import — pick one convention and use it consistently for the rest of the plan; recommend co-located per-component CSS files imported by each component, since it keeps `styles.css` to just the shared tokens)**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/palette/ToolRow.tsx apps/wield/src/palette/ToolRow.test.tsx apps/wield/src/palette/SearchView.tsx apps/wield/src/palette/SearchView.test.tsx apps/wield/src/palette/*.css
git commit -m "feat(app-ui): ToolRow and SearchView"
```

---

## Task 8: `useKeyboardNav`

**Files:**
- Create: `apps/wield/src/useKeyboardNav.ts`, `apps/wield/src/useKeyboardNav.test.ts`

**Interfaces:**

```typescript
export interface KeyboardNavOptions {
  itemCount: number;
  selectedIndex: number;
  onSelectIndex: (i: number) => void;
  onActivate: () => void; // Enter
  onEscape: () => void;
}

/** Attaches a document-level keydown listener for the lifetime of the component using it. */
export function useKeyboardNav(options: KeyboardNavOptions): void;
```
`ArrowDown`/`ArrowUp` move `selectedIndex` by ±1, clamped to `[0, itemCount - 1]` (no wrap — spec doesn't call for wrap-around; clamping is simpler and equally standard). `Enter` calls `onActivate()`. `Escape` calls `onEscape()`. All three `preventDefault()` so the browser doesn't scroll or do anything else. No-ops (no listener effect) when `itemCount === 0` for the arrow keys, but `Escape`/`Enter` still fire.

- [x] **Step 1: Write the failing test**

```typescript
import { renderHook } from "@testing-library/react";
import { test, expect, vi } from "vitest";
import { useKeyboardNav } from "./useKeyboardNav";

function fire(key: string) {
  window.dispatchEvent(new KeyboardEvent("keydown", { key, cancelable: true }));
}

test("arrow keys move selection, clamped to the list bounds", () => {
  const onSelectIndex = vi.fn();
  renderHook(() => useKeyboardNav({ itemCount: 3, selectedIndex: 1, onSelectIndex, onActivate: vi.fn(), onEscape: vi.fn() }));
  fire("ArrowDown");
  expect(onSelectIndex).toHaveBeenCalledWith(2);
  fire("ArrowUp");
  expect(onSelectIndex).toHaveBeenCalledWith(0);
});

test("ArrowDown at the last item does not go out of bounds", () => {
  const onSelectIndex = vi.fn();
  renderHook(() => useKeyboardNav({ itemCount: 2, selectedIndex: 1, onSelectIndex, onActivate: vi.fn(), onEscape: vi.fn() }));
  fire("ArrowDown");
  expect(onSelectIndex).toHaveBeenCalledWith(1);
});

test("Enter and Escape call their handlers", () => {
  const onActivate = vi.fn();
  const onEscape = vi.fn();
  renderHook(() => useKeyboardNav({ itemCount: 1, selectedIndex: 0, onSelectIndex: vi.fn(), onActivate, onEscape }));
  fire("Enter");
  fire("Escape");
  expect(onActivate).toHaveBeenCalled();
  expect(onEscape).toHaveBeenCalled();
});
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/useKeyboardNav.ts apps/wield/src/useKeyboardNav.test.ts
git commit -m "feat(app-ui): keyboard navigation hook"
```

---

## Task 9: `FormField` + `ArgForm`

**Files:**
- Create: `apps/wield/src/palette/FormField.tsx`, `apps/wield/src/palette/ArgForm.tsx`, tests

**Interfaces:**

```typescript
// FormField.tsx
export interface FormFieldProps {
  spec: ArgSpec;
  value: unknown;
  onChange: (value: unknown) => void;
}
export function FormField(props: FormFieldProps): JSX.Element;
```
Switches on `spec.arg_type` per the design spec §5 table:
- `{ File: { multiple } }` → a button showing the chosen filename (or "Choose a file…"); `onClick` calls `@tauri-apps/plugin-dialog`'s `open({ multiple, filters: spec.arg_type.File.filters.map(f => ({ name: f.label, extensions: f.extensions })) })` and calls `onChange` with the result (string or string[] or null on cancel — cancel leaves the value unchanged).
- `"Dir"` → same pattern with `open({ directory: true })`.
- `"Str" | "Text"` → `<input>` / `<textarea>`, `onChange` passes `e.target.value`.
- `{ Int: { range, step } }` → `<input type="number">` with `min`/`max` from `range` (if not null) and `step` (default `1`), `onChange` passes `Number(e.target.value)`.
- `{ Float: { range } }` → same, `step="any"`.
- `"Bool"` → `<input type="checkbox">`, label to the right, `onChange` passes `e.target.checked`.
- `{ Enum: { options } }` → `<select>`, one `<option>` per entry, `onChange` passes `e.target.value`.

```typescript
// ArgForm.tsx
export interface ArgFormProps {
  tool: ToolSummary;
  initialValues: Record<string, unknown>; // for "Run again" — retained values; {} for a fresh open
  onSubmit: (values: Record<string, unknown>) => void;
  onEscape: () => void;
}
export function ArgForm(props: ArgFormProps): JSX.Element;
```
Computes visible fields client-side mirroring `wield-core`'s `visible_args` rule (Task 9 Step 3 implements this as a small pure helper, `isVisible(spec, values, allSpecs)`, exported for its own unit tests): a spec with `when: null` is always visible; with `when: { arg, in: [] }` (the presence form) visible iff `values[arg]` is set; with `when: { arg, in: [...] }` visible iff `values[arg]` matches one of the literals **and** the referenced spec is itself visible. Only visible fields render, in `args` order, each as one `FormField` with a `<label>` above it (spec's `.field-name` styling — see the brainstorm mockup, `field-name` not `label` class to avoid any future collision). Submit button text is the tool's title's verb-ish first word where sensible, but per YAGNI just use the tool's title as the label is fine — **use `tool.title`** as the button text (matches the locked mockup's "Convert" button, since `image.convert`'s title is "Convert image" — actually the mockup showed the bare verb "Convert"; since deriving a verb from an arbitrary title is unreliable string-munging, use the literal `tool.title` as the button text instead ("Convert image") and don't try to shorten it — note this as a deliberate, small deviation from the mockup's exact copy, not a placeholder).

- [x] **Step 1: Write the failing tests**

`FormField.test.tsx` — one test per `ArgType` branch (7 tests: File, Dir, Str, Int, Float, Bool, Enum), each rendering the field and firing a change, asserting `onChange` received the right coerced value. Mock `@tauri-apps/plugin-dialog`'s `open` for the File/Dir cases.

`ArgForm.test.tsx`:
```typescript
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { test, expect, vi } from "vitest";
import { ArgForm, isVisible } from "./ArgForm";

const widthSpec = { name: "width", label: "Width", help: null, arg_type: { Int: { range: null, step: null } }, default: null, required: false, when: { arg: "format", in: [] } };

test("isVisible: presence form is satisfied by any value on the referenced arg", () => {
  expect(isVisible(widthSpec, {}, [widthSpec])).toBe(false);
  expect(isVisible(widthSpec, { format: "webp" }, [widthSpec])).toBe(true);
});

test("renders only visible fields and submits their values", async () => {
  const tool = {
    id: "image.convert", title: "Convert image", keywords: [], category: "Convert" as const, available: true, reason: null,
    args: [
      { name: "format", label: "Output format", help: null, arg_type: { Enum: { options: ["png", "webp"] } }, default: { Str: "png" }, required: true, when: null },
    ],
  };
  const onSubmit = vi.fn();
  render(<ArgForm tool={tool} initialValues={{}} onSubmit={onSubmit} onEscape={vi.fn()} />);
  await userEvent.click(screen.getByRole("button", { name: "Convert image" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ format: "png" }));
});

test("initialValues pre-fills the form for Run again", () => {
  const tool = { id: "x", title: "X", keywords: [], category: "Convert" as const, available: true, reason: null,
    args: [{ name: "note", label: "Note", help: null, arg_type: "Str" as const, default: null, required: false, when: null }] };
  render(<ArgForm tool={tool} initialValues={{ note: "hello" }} onSubmit={vi.fn()} onEscape={vi.fn()} />);
  expect(screen.getByDisplayValue("hello")).toBeInTheDocument();
});
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement `FormField.tsx`, `ArgForm.tsx` (incl. exported `isVisible`)**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/palette/FormField.tsx apps/wield/src/palette/FormField.test.tsx apps/wield/src/palette/ArgForm.tsx apps/wield/src/palette/ArgForm.test.tsx apps/wield/src/palette/*.css
git commit -m "feat(app-ui): generated argument form"
```

---

## Task 10: `RunningView`

**Files:**
- Create: `apps/wield/src/palette/RunningView.tsx`, test

**Interfaces:**

```typescript
export interface RunningViewProps {
  toolTitle: string;
  progress: Progress | null; // most recent event received, or null before the first one
}
export function RunningView(props: RunningViewProps): JSX.Element;
```
Renders a spinner + `"{verb-ish message}"` — use `Converting {toolTitle}…"`-style text is over-specific per tool; simplest correct copy: `${toolTitle}…` (e.g. "Convert image…") which reads fine for every current and future tool without per-tool copy logic. Below it: a progress bar (`--accent` fill) only when `progress` is `{ Percent: n }` (width `${n}%`); when `progress` is `{ Message: m }`, show `m` as the line instead of the tool title suffix; otherwise (null/`Started`) show just the spinner, no bar. Footer hint: "esc to cancel".

- [x] **Step 1: Write the failing tests**

```typescript
import { render, screen } from "@testing-library/react";
import { test, expect } from "vitest";
import { RunningView } from "./RunningView";

test("shows a percent bar when progress reports one", () => {
  render(<RunningView toolTitle="Convert image" progress={{ Percent: 62 }} />);
  const bar = screen.getByTestId("progress-fill");
  expect(bar).toHaveStyle({ width: "62%" });
});

test("shows the message text when progress is a Message", () => {
  render(<RunningView toolTitle="Convert image" progress={{ Message: "Encoding…" }} />);
  expect(screen.getByText("Encoding…")).toBeInTheDocument();
});

test("shows just the tool title with no bar before any progress arrives", () => {
  render(<RunningView toolTitle="Convert image" progress={null} />);
  expect(screen.getByText("Convert image…")).toBeInTheDocument();
  expect(screen.queryByTestId("progress-fill")).not.toBeInTheDocument();
});
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/palette/RunningView.tsx apps/wield/src/palette/RunningView.test.tsx apps/wield/src/palette/*.css
git commit -m "feat(app-ui): running view with progress"
```

---

## Task 11: Outcome components + `ResultView`

**Files:**
- Create: `apps/wield/src/palette/outcomes/{ValueResult,FileResult,ReportResult,UnavailableResult,FailedResult}.tsx`, `apps/wield/src/palette/ResultView.tsx`, tests

**Interfaces:**

```typescript
// each outcome component takes exactly its own payload shape, e.g.:
export function ValueResult(props: { kind: ValueKind; data: string; onCopy: () => void }): JSX.Element;
export function FileResult(props: { path: string; onOpenFolder: () => void; onRunAgain: () => void }): JSX.Element;
export function ReportResult(props: { title: string; lines: string[] }): JSX.Element;
export function UnavailableResult(props: { reason: string; fix: string | null }): JSX.Element;
export function FailedResult(props: { stage: Stage; detail: string; hint: string | null; onRetry: () => void; onCopyDetails: () => void }): JSX.Element;

export interface ResultViewProps {
  outcome: ToolOutcome; // never "Cancelled" here — App.tsx handles Cancelled by not entering the result view at all (spec §3.11)
  onRunAgain: () => void;
  onRetry: () => void;
}
export function ResultView(props: ResultViewProps): JSX.Element;
```
`ResultView` switches on the outcome's tag (`typeof outcome === "string"` is unreachable here per the contract above — if it ever is `"Cancelled"`, render nothing and log a dev warning, since that indicates a caller bug) and renders the matching component, wiring:
- `ValueResult.onCopy` → `navigator.clipboard.writeText(data)`.
- `FileResult.onOpenFolder` → `@tauri-apps/plugin-opener`'s `revealItemInDir(path)`; `onRunAgain` → `props.onRunAgain()`.
- `FailedResult.onRetry` → `props.onRetry()`; `onCopyDetails` → `navigator.clipboard.writeText(detail)`.

- [x] **Step 1: Write the failing tests** — one rendering test per component (5 files) asserting the key text/values show and the right callback fires on its trigger, plus a `ResultView.test.tsx` with one case per `ToolOutcome` tag asserting the right child component's distinguishing text appears (e.g. the `Value` case renders the hex string, the `Failed` case renders `detail`).

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement all six files**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/palette/outcomes apps/wield/src/palette/ResultView.tsx apps/wield/src/palette/ResultView.test.tsx
git commit -m "feat(app-ui): result cards for every ToolOutcome"
```

---

## Task 12: `App.tsx` — the state machine

**Files:**
- Replace: `apps/wield/src/App.tsx`
- Delete: `apps/wield/src/App.test.tsx` (replaced by Task 13's file)

**Interfaces:**

```typescript
type View =
  | { kind: "search" }
  | { kind: "form"; tool: ToolSummary; values: Record<string, unknown> }
  | { kind: "running"; tool: ToolSummary; runId: RunId | null; progress: Progress | null }
  | { kind: "result"; tool: ToolSummary; outcome: Exclude<ToolOutcome, "Cancelled">; lastValues: Record<string, unknown> };

// Actions the reducer handles: SET_QUERY, SET_TOOLS, SELECT_INDEX, OPEN_FORM, RUN,
// PROGRESS, FINISHED (outcome), ESCAPE, RUN_AGAIN, RETRY.
```
Behaviour, precisely per the design spec §3/§4:
- On mount and on every `query` change (debounced ~120ms so keystrokes don't each trigger a round trip), call `listTools(query || undefined)`; when `query` is empty, sort/filter the full list down to `getRecentIds()` order (falling back to the full list if there are no recents yet) and set `showingRecents = true`; otherwise use the backend's ranked results as-is and `showingRecents = false`.
- `useKeyboardNav` is active only while `view.kind === "search"`, wired to the current tool list.
- Activating a row (`Enter` or click) on an **available** tool: if `tool.args.length === 0`, go straight to `{ kind: "running", ... }` and call `runTool`; else go to `{ kind: "form", tool, values: {} }`. Activating an **unavailable** row is a no-op (nothing to run).
- `ArgForm`'s submit → `bumpRecent(tool.id)`, transition to `running`, call `runTool(tool.id, values, onProgress)`. `onProgress` dispatches `PROGRESS`. The promise resolving dispatches `FINISHED` with the outcome — if it's `"Cancelled"`, go straight back to `{ kind: "search" }` (no result view, per spec §3.11); otherwise go to `{ kind: "result", tool, outcome, lastValues: values }`.
- `Escape` handling exactly per spec §4: `form` → `search` (query untouched); `running` → call `cancelRun(runId)` then wait for the in-flight promise's `Cancelled` resolution to actually transition (don't optimistically transition before the backend confirms — avoids a race where a `FINISHED` with a real outcome arrives after you've already left); `result` reached via a normal run → `search`; `result` reached via **Run again** → back to `{ kind: "form", tool, values: lastValues }` (this needs the reducer to remember *how* the current result was reached — simplest: `RUN_AGAIN` sets a flag/marker consumed by the next `Escape` from `result`, e.g. store `viaRunAgain: boolean` on the `result` view state); at `search` with `query === ""`, call `hidePalette` (import `@tauri-apps/api/window` or reuse an existing `invoke("hide_palette")` — **note:** `hide_palette` is a P5a command taking `AppHandle`, no args — call it via `lib/wield.ts`'s pattern, add a one-line `hidePalette()` export there too as part of this task).
- `FileResult`'s "Run again" and `FailedResult`'s "Retry" both dispatch `RUN_AGAIN` (retry reuses the exact same values and re-runs immediately without showing the form again — matches spec §3.10 "retry … re-runs with the same args, skipping the form"; "Run again" on a File result goes back to the *form* per spec §3.7 wording "Run again returns to the form with values retained" — these are two different UX flows despite similar names: **Failed → Retry = re-run immediately; File/Value → Run again = reopen the form**. Encode this as two distinct reducer actions, `RETRY` (re-run) and `RUN_AGAIN` (reopen form), not one.).

- [x] **Step 1–4: TDD this incrementally** — this task is large enough that "write one test, watch it fail, implement, watch it pass" applies at the level of *one reducer transition at a time*, not the whole component in one shot. Suggested order, each its own red/green cycle within this task (not separate top-level tasks — Task 13 is the outer integration-test net; these are the inner loop building `App.tsx` itself):
  1. Renders `SearchView` initially, `listTools()` called on mount.
  2. Selecting a no-arg tool (`color.pick`) runs immediately (mock `runTool`).
  3. Selecting `image.convert` opens the form; submitting it calls `runTool` and transitions through `running` to `result`.
  4. A `Cancelled` outcome returns to `search`, not `result`.
  5. Escape from `form` → `search`; Escape from `result` (normal) → `search`; Escape from `result` (via Run again) → `form` with `lastValues`.
  6. Progress events update the `running` view live.
  Use the existing `App.test.tsx`-style `vi.mock("./lib/wield")` (mock the whole module, not `@tauri-apps/api/core` directly, now that everything goes through `lib/wield.ts`).

- [x] **Step 5: Commit**

```bash
git add apps/wield/src/App.tsx
git rm apps/wield/src/App.test.tsx
git commit -m "feat(app-ui): App state machine wiring search, form, run, and results"
```

---

## Task 13: End-to-end integration tests

**Files:**
- Create: `apps/wield/src/App.test.tsx`

**Interfaces:** A handful of full-flow tests exercising `App` with `lib/wield.ts` mocked, covering the scenarios Task 12 built incrementally but now as a black-box regression net:

1. Type a query → filtered results render in rank order.
2. Full happy path: select `image.convert` → fill the form → submit → see progress → see the `File` result → click "Run again" → form is pre-filled → submit again.
3. Full failure path: run a tool → `Failed` outcome → click "Retry" → `runTool` called again with the same args, no form shown in between.
4. Cancellation: start a run, call the mocked `cancelRun`, resolve `runTool`'s promise with `"Cancelled"` → back at search, no result card rendered.
5. Escape stepping matrix from spec §4, as one test walking: search → form → escape → search → form (again) → run → escape (cancels) → search.
6. An unavailable tool's row shows its reason and Enter does nothing (list state unchanged).

- [x] **Step 1: Write these six tests against the Task 12 implementation** (they should mostly pass immediately if Task 12's inner loop was thorough — this task is the "did we actually wire it all together right" check, expect to find and fix a few gaps, which is normal and not a plan failure).
- [x] **Step 2: Run — fix any gap found, don't skip**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && npm run test -w apps/wield`
Expected: PASS, all 6 plus everything from Tasks 4–11.

- [x] **Step 3: Commit**

```bash
git add apps/wield/src/App.test.tsx
git commit -m "test(app-ui): end-to-end palette flow coverage"
```

---

## Task 14: Workspace green + P6a wrap-up

- [x] **Step 1: Full checks**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check
```

- [x] **Step 2: Manual smoke (if a display is available)**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
npm run dev:app -w apps/wield &   # or: (cd apps/wield && npm run dev)
```
Trigger `show_palette` (e.g. the tray, if one built in P5b renders here) and walk the flow by hand: search "color", run `color.pick`, search "convert", fill and run `image.convert` against a real small PNG, confirm the result path opens, confirm Escape/cancel behave as designed. If no display/tray interaction is possible in this environment, say so plainly in the report rather than claiming it was checked (same honesty standard as P5a/P5b's launch smoke caveats).

- [x] **Step 3: Report to GEON**

```
agent-comms message post --to GEON --kind FYI --subject "P6a complete" --body "Palette UI landed on feat/p6a-palette-ui: search+recents, keyboard nav, generated arg form (incl. the When presence form client-side), running view with live progress (new Channel<Progress> on run_tool), and all 5 ToolOutcome result cards, near-black+teal design system per the approved spec. list_tools gained a search query + reason field. cargo test --workspace + clippy + npm run check green. Manual UI smoke: <done | not possible, no display>. N commits, branch pushed."
```

- [x] **Step 4: Commit the plan + push**

```bash
git add docs/superpowers/plans/2026-09-11-wield-m1-p6a-palette-ui.md
git commit -m "docs: mark M1 P6a plan complete"
git push -u origin feat/p6a-palette-ui
```

GEON opens the PR, reviews, merges. **P6b** (Preferences window content, System Status, the `tray://select-tool` listener) is written just-in-time.

---

## Self-Review

**Spec coverage:**

- §2 (colour/type/layout/motion tokens) → Task 6, referenced by every component task ✓
- §3 all 11 states → Tasks 7 (1–3), 9 (4), 10 (5), 11 (6–11) ✓
- §4 keyboard/Escape model, including the Run-again-vs-Retry distinction → Task 12 ✓ (explicitly disambiguated, since the spec's wording could be conflated)
- §5 component breakdown → File Structure mirrors it exactly; `list_tools` query param, `run_tool` Channel, dialog/opener plugins → Tasks 1–3 ✓
- §5 testing strategy (hook test, per-ArgType tests, per-outcome tests, integration tests, recents tests) → Tasks 4–13 ✓ one-to-one
- §6 accessibility (focus-visible, reduced motion, contrast, aria-live) → Task 6 (focus/motion), Task 11 (aria-live on `ResultView`/`RunningView` — **add this explicitly to Task 10/11's implementation, not just Task 6's CSS** — noted here since the interfaces above didn't spell out the `aria-live="polite"` wrapper explicitly enough; the implementer must add `aria-live="polite"` to the container `RunningView` and `ResultView` render into)

**Placeholder scan:** no TBD/TODO. The button-copy deviation from the mockup's bare "Convert" to the full `tool.title` is explained as a deliberate simplification, not left vague. `ToolRow`'s icon is explicitly "blank chip, icons are later polish" — named, not hidden.

**Type consistency:** `ToolSummary`/`ArgSpec`/`ArgType`/`ToolOutcome`/`Progress`/`RunId` defined once in `lib/wield.ts` (Task 4) and used identically by every component task through Task 13 — verified the field names/shapes against the actual Rust source (`commands.rs`, `descriptor.rs`, `outcome.rs`) while writing this plan, not assumed. `isVisible` (Task 9) mirrors `wield-core::args::visible_args`'s presence-form rule (P4) exactly, including the "referenced spec must itself be visible" chaining rule.

**Ordering:** 1–2 (backend, independent of each other, both needed before 4) → 3 (plugins, independent) → 4 (types/IPC, needs 1+2's shapes) → 5 (recents, independent) → 6 (tokens, independent) → 7 (needs 4) → 8 (independent) → 9 (needs 4, 3 for the dialog plugin) → 10 (needs 4) → 11 (needs 4, 3 for opener) → 12 (needs 4, 5, 7, 8, 9, 10, 11 — the integration point) → 13 (needs 12) → 14. Consistent; 3/5/6/8 can run in any order relative to each other.

---

## Execution note

P6a makes the palette real. **P6b** (written just-in-time after this merges) adds Preferences window content (hotkey status + fallback command, System Status from `capabilities`, palette behaviour toggles), wires the `tray://select-tool` event P5b already emits into `App.tsx`'s `OPEN_FORM`/`RUN` actions, and is the natural home for tool icons if this plan's blank chips prove too plain. P7 (Flatpak + CI) is last in M1.
