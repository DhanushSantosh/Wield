# Wield P6a — Palette UI Design

**Date:** 2026-09-11
**Status:** Approved for planning (visual direction confirmed interactively; functional details below)

## 1. What this covers

The first real content for `apps/wield/src` — the palette window's React UI. Search, keyboard navigation, the generated argument form, running a tool with live progress, and the result card. This replaces the P1 placeholder screen inside the `palette` window built in P5a/P5b.

**Not covered here** (separate plans): Preferences window content, System Status page, first-run onboarding — **P6b**. Tray/hotkey/single-instance/window management — already built (P5a/P5b). Flatpak packaging, CI — P7.

## 2. Visual design system

Established interactively (four rounds of comparison, converging on a single accent over near-black).

### Color

| Token | Value | Use |
| --- | --- | --- |
| `--bg` | `#0a0a0a` | Palette window background |
| `--surface` | `#161616` | Icon chips, input controls, the running-state block |
| `--border` | `#1c1c1c` | Hairline dividers, card outline |
| `--text` | `#e8e8e8` | Primary text |
| `--text-muted` | `#6b6b6b` | Placeholder, category label, help text, secondary lines |
| `--text-faint` | `#4a4a4a` | Section labels ("Recent") |
| `--accent` | `#3ed0c4` | Selection tint/bar, run button, progress fill, spinner, copyable-value color |
| `--accent-ink` | `#0c1a18` | Text color *on* the accent (run button label) |
| `--danger` | `#d9714f` | Error dot / failed-outcome accent (muted terracotta-red, not a saturated alarm red) |

One accent, used consistently: the selected row's left bar + background tint, the primary action button, the progress bar/spinner, and the color of a copyable technical value (hex code, path) all use `--accent`. Nothing else is colored — category is shown as plain muted text, not a color, now that a single accent carries the "this is active/interactive" meaning.

### Type

- **UI face:** Inter (system sans fallback: `-apple-system, "Segoe UI", sans-serif`). One family for everything — labels, body, buttons.
- **Data face:** `ui-monospace, "JetBrains Mono", monospace` — used **only** for values a user would copy or that are inherently technical: hex/rgb/hsl, file paths, exit-code detail text. Never for labels, categories, or section headers.
- Scale: input `15.5px`, row/body `14px`, secondary/meta `11–12.5px`. No uppercase tracking anywhere.

### Layout & elevation

- One floating surface: `border-radius: 15px`, `box-shadow: 0 20px 50px rgba(0,0,0,.6), 0 0 0 1px var(--border)`. Not a stack of separately-shadowed cards.
- Result rows are flat — no per-row border or shadow. Selection is a background tint + a 2px left accent bar, nothing else moves or gains a shadow.
- Max width ~460–520px, centered, matches the P5a window config (`720×480`, resizable: false — the content fits inside that, doesn't resize the window).

### Motion

One deliberate transition: the window's content height animates when the view changes (search → arg-form → running → result), via a simple max-height/opacity transition on the content container. No per-row hover animation, no staggered entrances. Respect `prefers-reduced-motion` (skip the height transition, snap instead).

## 3. States

1. **Opened, empty query** — "Recent" section label (faint), most-used tools listed below it (ranked by local recency, stored in `localStorage` — see §5).
2. **Typing** — query sent to the backend's fuzzy search (§5); results replace the recent list live, no section label.
3. **Row selected** (keyboard-driven, always exactly one selected row while the list has focus) — accent tint + left bar. Unavailable tools appear dimmed (`opacity: .45`), not hidden, with their `capabilities`/`list_tools`-provided reason as the row's secondary text instead of the category.
4. **Argument form** — chosen when Enter is pressed on a tool with `args.length > 0`. The palette's content area replaces the result list with a stacked form generated from `ArgSpec[]` (§6). A tool with zero args or whose only args are all satisfied by defaults skips straight to running.
5. **Running** — spinner + a progress bar (percent fill if the backend reports one via `Progress::Percent`, otherwise the bar is omitted and only the spinner + a message line show). "esc to cancel" hint.
6. **Result — Value** — the value plus (for `ValueKind::Color`) the parsed hex/rgb/hsl already embedded in `data` (P3's JSON payload) rendered as three lines, monospace. Enter copies the primary value (hex) to the clipboard.
7. **Result — File** — the output path, monospace, with two actions: "open folder" (reveals it in the file manager) and "run again" (see §4).
8. **Result — Report** — title + lines, plain text, no monospace unless a line looks like a path/value (out of scope to auto-detect in P6a — always plain).
9. **Result — Unavailable** — same visual slot as Failed, but non-technical: the `reason` and `fix` text directly (already user-facing language from the backend), `--text-muted` not `--danger` (it's not an error, it's "not set up yet").
10. **Result — Failed** — `--danger` dot + one-line `detail`, `hint` (if present) as the muted second line, "retry" (re-runs with the same args — jumps back to step 5, skipping the form) and "copy details" actions.
11. **Result — Cancelled** — no card at all. Silent return to the search state (spec §7: user-cancel is always silent).

## 4. Interaction model

- **Arrow Up/Down** — move selection within the current list (results or, later, a multi-field form's focus order follows normal Tab order instead).
- **Enter** — on a search row: run immediately (no-arg tool) or open the arg form (has args). On the arg form: run. On a result: no default action (each result's primary action, if any, is stated in its own hint — e.g. Value's "↵ copy hex").
- **Escape** — steps back one level: arg-form → search (query retained); running → cancels the run and returns to search; result → search, **except** when reached via "Run again", where the natural next Escape returns to the arg-form with values retained (matches spec: "Run again returns to the form with values retained"). At the top level (plain search, nothing typed distinguishing it from just-opened), Escape calls `hide_palette`.
- **Blur** (window loses focus) — calls `hide_palette`. Always on for P6a; a toggle for this lives in Preferences (P6b) but the code path is the same `hide_palette` call either way, so nothing here is blocked on P6b.
- No other global shortcuts in P6a (no Cmd+K-style intra-app palette-of-palettes, no per-category jump keys) — YAGNI until a real need shows up.

## 5. Architecture

### Component breakdown (`apps/wield/src/`)

```
main.tsx                 — unchanged entry point
App.tsx                  — top-level state machine (which view is active) + keyboard listener
palette/
  SearchView.tsx          — query input + result rows + recents
  ToolRow.tsx              — one row (icon glyph placeholder, name, category/reason text)
  ArgForm.tsx               — renders one FormField per visible ArgSpec (When-visibility client-side, mirroring wield-core's rule: empty `in` = "arg is set")
  FormField.tsx             — one control per ArgType (see below)
  RunningView.tsx            — spinner + progress bar + cancel hint
  ResultView.tsx              — dispatches on ToolOutcome tag to one of:
  outcomes/
    ValueResult.tsx
    FileResult.tsx
    ReportResult.tsx
    UnavailableResult.tsx
    FailedResult.tsx
lib/
  wield.ts                — typed wrappers over every `invoke("...")` call + the Channel subscription
  recents.ts               — localStorage-backed recent-tool-id list (read/bump/list, capped at 8)
useKeyboardNav.ts (hook)  — arrow/enter/escape handling shared by SearchView
```

State lives in `App.tsx` via `useReducer` (no state library — the state machine is small and fully enumerable: `{ view: "search" | "form" | "running" | "result", toolId, args, runId, progress, outcome }`). No global store needed for a single-window app.

### `FormField` mapping (`ArgType` → control)

| `ArgType` | Control |
| --- | --- |
| `File { multiple: false }` | Button "Choose a file…" → `@tauri-apps/plugin-dialog`'s `open()`; shows the chosen filename once set |
| `File { multiple: true }` | Same button, `open({ multiple: true })`; shows a count once chosen |
| `Dir` | Button "Choose a folder…" → `open({ directory: true })` |
| `Str` / `Text` | Text input (`Text` gets a slightly taller, multi-line `<textarea>`) |
| `Int { range, step }` | Number input; `min`/`max`/`step` set from `range`/`step` when present |
| `Float { range }` | Number input, `step="any"`, `min`/`max` from `range` |
| `Bool` | Checkbox, label to the right (no separate "control" chrome — the row *is* the control) |
| `Enum { options }` | `<select>` populated from `options` |

**New dependency:** `@tauri-apps/plugin-dialog` (+ its Rust half `tauri-plugin-dialog` registered in `wield-app`). On Linux this goes through the desktop's native file chooser, which is portal-backed (`rfd`/GTK use `org.freedesktop.portal.FileChooser` under the hood) — consistent with the spec's "files enter through the FileChooser portal" even though this isn't `wield-portal`'s own `ashpd` wrapper. No new `wield-portal` code needed for this in P6a.

**New dependency:** `@tauri-apps/plugin-opener` (+ `tauri-plugin-opener`) for the File result's "open folder" action (`revealItemInDir`).

### Backend changes needed (small, additive — in this plan, not a separate one)

1. **Search.** `list_tools` gains an optional `query: Option<String>` param. `None`/empty → `registry.list()` (today's behavior, unchanged default). `Some(q)` → `registry.search(q)` (already implemented, P2/P4) mapped to `ToolSummary` in the existing rank order. No new command — this keeps the frontend's one "get tools" call doing double duty, matching how the CLI and tray already just read `list_tools_impl`.
2. **Progress streaming.** `run_tool` gains a `tauri::ipc::Channel<wield_core::Progress>` parameter. The frontend creates a channel per run and passes it; the backend forwards every `Progress` it currently drains-and-discards (P5a) onto the channel instead, in addition to still returning the final `RunResult` when the executor completes. `run_tool_impl`'s signature changes to accept an `impl Fn(Progress) + Send + Sync` (or the channel directly) so it stays testable without a real Tauri channel — same "inject the effectful call" shape used throughout `wield-core`/`wield-portal`.
3. Everything else (`cancel`, `capabilities`, `hotkey_status`) is used as-is.

### Testing

Vitest + Testing Library (already set up, P1). `@tauri-apps/api/core` and the two new plugins mocked at the module boundary, same pattern as P1's `App.test.tsx`.

- `useKeyboardNav` — arrow wrap-around, Enter dispatch, Escape level-stepping — pure hook test.
- `ArgForm` / `FormField` — one test per `ArgType` mapping, `when`-visibility (including the presence form).
- `ResultView` — one test per `ToolOutcome` variant renders the right outcome component.
- `App` state machine — a handful of integration tests driving `invoke` mocks through search → form → run (mocked progress events → result) → run-again, and the Escape-stepping matrix from §4.
- `recents.ts` — localStorage read/bump/cap unit tests.

No Rust-side test changes beyond the two additive command changes above (covered by their own unit tests, same TDD pattern as every prior plan).

## 6. Accessibility

- Every interactive element reachable by keyboard alone (arrow list + Tab within the form); visible focus ring using `--accent` at reduced opacity, never relying on color alone (also a left-bar / outline shape change).
- `prefers-reduced-motion` skips the height/opacity transition.
- Text contrast: `--text` (`#e8e8e8`) on `--bg` (`#0a0a0a`) and `--accent-ink` (`#0c1a18`) on `--accent` (`#3ed0c4`) both clear WCAG AA for normal text.
- Result and running regions use `aria-live="polite"` so a screen reader announces outcome changes without the user needing to navigate to them.

## 7. Self-review

- **Placeholder scan:** no TBD/TODO. The one explicitly-deferred item (auto-detecting path-like Report lines for monospace) is named as out-of-scope, not left vague.
- **Consistency:** the four visual states shown interactively (idle/recents, arg-form, value result, failed result) match §3 exactly — same field labels, same button text ("Convert", not "Run"), same hint style.
- **Type consistency:** `ToolSummary`, `ArgSpec`, `ArgType`, `ToolOutcome`, `Progress` all named exactly as they exist in `wield-core`/the P5a commands today (verified against `apps/wield/src-tauri/src/commands.rs` and `wield-core/src/{descriptor,outcome}.rs` while writing this) — the implementation plan can use these names directly.
- **Scope check:** this is one cohesive surface (the palette) sized for one implementation plan. Preferences/System Status are explicitly out (§1) — P6b.
