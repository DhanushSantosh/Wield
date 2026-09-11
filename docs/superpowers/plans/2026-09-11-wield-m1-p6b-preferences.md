# Wield M1 · Plan P6b — Preferences, System Status, tray tool selection — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the (currently content-less, P5b) Preferences window real content — hotkey status with the copyable fallback command, a palette-behavior toggle (blur-to-hide), and a System Status page reading `capabilities` — and wire P5b's `tray://select-tool` event into the palette so clicking a tool in the tray menu actually opens it.

**Architecture:** `apps/wield/src` still has one Vite entry (`index.html`/`main.tsx`); `main.tsx` now branches on the current Tauri window's label (`palette` → `<App />`, `preferences` → `<Preferences />`) rather than gaining a second HTML entry point — the simplest option that needs no build-tooling changes. `Preferences.tsx` is three sections (Hotkey, Palette behavior, System Status) built from two commands that already exist and are already registered (`hotkey_status`, `capabilities` — no backend changes needed in this plan at all). The blur-to-hide toggle is a `localStorage` setting (same pattern as P6a's recents), read by `App.tsx`'s existing blur listener. The tray's `tray://select-tool` event is a `@tauri-apps/api/event` `listen()` call in `App.tsx` that reuses the exact same "activate a tool" path a search-row click already uses.

**Tech Stack:** React 19, TypeScript, Vite, Vitest + Testing Library — no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-10-wield-design.md` §5 "Preferences" (hotkey setup, palette behaviour, System Status page) and "the tray … route through it" (tool selection). `docs/superpowers/specs/2026-09-11-wield-p6a-palette-design.md`'s §2 design tokens apply unchanged — Preferences uses the same dark+teal system, not a different look.

---

## GEON amendment — 2026-09-11

- **Scope, deliberately narrow.** In scope: Hotkey status display, one palette-behaviour toggle (blur-to-hide), System Status (binaries/portals/per-tool availability), tray tool-selection wiring. **Out of scope, named so nobody re-derives them mid-task:**
  - **Launch-at-login.** Needs a new `wield-portal` `Background` portal wrapper (same shape of work as P5b's `GlobalShortcuts` module) plus first-run-onboarding UX the spec explicitly separates out. A later plan, not P6b.
  - **Per-tool enable/disable.** No backend concept of a "disabled" tool exists (the registry has no such flag); wiring it means touching `wield-core`, which this plan's scope forbids. Later plan.
  - **Default output directory override.** `OutputDir` today is `SameAsInput` only; a user-configurable default needs new `wield-core`/`wield-tools` work. Later plan.
  - **Hotkey rebinding UI.** The fallback text is the only affordance in P6b — actually changing the bound trigger is a `GlobalShortcuts` `configure_shortcuts` call (ashpd exposes it, per P5b's module notes) that nobody has designed the UX for yet.
- **No backend changes.** `hotkey_status` and `capabilities` are already implemented and already registered in `invoke_handler!` (P5a/P5b) — verify this before starting (Task 1 Step 0), don't re-add them.
- **Branch:** `feat/p6b-preferences` off `master`, per-task commits, push at the end, do **not** open the PR.
- **`cargo` is not on the default `PATH`.** `export PATH="$HOME/.cargo/bin:$PATH"` before every cargo invocation (only needed here for `cargo fmt`/`clippy`/`test` on the otherwise-unchanged Rust side — Task 10's wrap-up).
- **Mechanical-correction rule (as every prior plan):** exact `@tauri-apps/api/event`/`@tauri-apps/api/window` API names may drift — check what's installed and adapt; note it in the Task 10 report.
- **`npm run check`** green at the end of every task that touches buildable code.

---

## Global Constraints

- **Repo root:** `/home/dhanush/Projects/Wield` (branch `feat/p6b-preferences` off `master`).
- **Design tokens unchanged.** Every color/spacing value in Preferences comes from `apps/wield/src/styles.css`'s existing `:root` custom properties (P6a) — no new tokens, no different palette for "the settings window."
- **Every `invoke`/`listen` call goes through `lib/wield.ts`** (extended, not bypassed) — same rule P6a established, for the same testability reason.
- **`npm run check`** green at the end of every task that touches buildable code.

---

## File Structure

**Modified:**

| File | Change |
| --- | --- |
| `apps/wield/src/main.tsx` | branch on window label; render `<App />` or `<Preferences />` |
| `apps/wield/src/lib/wield.ts` | add `hotkeyStatus()`, `capabilities()` + their TS types; add `onSelectTool(handler)` wrapping `listen("tray://select-tool", ...)` |
| `apps/wield/src/App.tsx` | blur listener respects the new setting; subscribes to `onSelectTool` and routes it through the existing activation path |

**Created:**

| File | Responsibility |
| --- | --- |
| `apps/wield/src/lib/settings.ts` | `getBlurToHide()` / `setBlurToHide()`, `localStorage`-backed, default `true` |
| `apps/wield/src/Preferences.tsx` | top-level Preferences window content |
| `apps/wield/src/preferences/HotkeySection.tsx` | registered / pending / unavailable+fallback-command display |
| `apps/wield/src/preferences/PaletteBehaviorSection.tsx` | the blur-to-hide checkbox |
| `apps/wield/src/preferences/SystemStatusSection.tsx` | binaries / portals / tools tables from `capabilities()` |
| `apps/wield/src/preferences/*.css` | co-located styles, same convention as `palette/*.css` |

**Test files** beside each source file, Vitest convention already established.

**Out of P6b scope:** everything in the GEON amendment's bulleted list above, plus any change to `apps/wield/src-tauri` (no backend work at all in this plan) or any other crate.

---

## Task 1: `lib/wield.ts` — `hotkeyStatus`, `capabilities`, `onSelectTool`

**Files:**
- Modify: `apps/wield/src/lib/wield.ts`, `apps/wield/src/lib/wield.test.ts`

**Interfaces:**
- **Step 0 (not a code step):** confirm `hotkey_status` and `capabilities` are registered in `apps/wield/src-tauri/src/lib.rs`'s `tauri::generate_handler!` list. They are, per P5a/P5b — if for any reason they are not, **stop and tell GEON**, do not add backend code yourself (out of this plan's scope).
- Types (mirroring the Rust serde output exactly — `HotkeyState` is `#[serde(tag = "state")]`, i.e. **internally** tagged, unlike every `wield-core` enum P6a dealt with; `CapabilitiesReport`/`ToolAvailability` are plain structs, default field-name serialization):

  ```typescript
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

  export async function capabilities(): Promise<CapabilitiesReport> {
    return invoke<CapabilitiesReport>("capabilities");
  }

  /** Subscribes to the tray's tool-selection event; returns an unsubscribe function. */
  export async function onSelectTool(handler: (toolId: string) => void): Promise<() => void> {
    const { listen } = await import("@tauri-apps/api/event");
    const unlisten = await listen<string>("tray://select-tool", (event) => handler(event.payload));
    return unlisten;
  }
  ```
  (`BTreeMap<String, T>` on the Rust side serializes as a plain JSON object — `Record<string, T>` is correct, not a `Map`.)

- [x] **Step 1: Write the failing tests**

Append to `wield.test.ts`:

```typescript
test("hotkeyStatus invokes hotkey_status", async () => {
  invokeMock.mockResolvedValueOnce({ state: "Registered" });
  expect(await hotkeyStatus()).toEqual({ state: "Registered" });
  expect(invokeMock).toHaveBeenCalledWith("hotkey_status");
});

test("capabilities invokes capabilities", async () => {
  const report = { binaries: { magick: true }, portals: { Screenshot: 2 }, tools: [] };
  invokeMock.mockResolvedValueOnce(report);
  expect(await capabilities()).toEqual(report);
  expect(invokeMock).toHaveBeenCalledWith("capabilities");
});

test("onSelectTool forwards the event payload and returns an unsubscribe fn", async () => {
  let captured: ((event: { payload: string }) => void) | undefined;
  const unlistenMock = vi.fn();
  vi.doMock("@tauri-apps/api/event", () => ({
    listen: vi.fn((_name: string, cb: typeof captured) => {
      captured = cb;
      return Promise.resolve(unlistenMock);
    }),
  }));
  const { onSelectTool: onSelectToolFresh } = await import("./wield");
  const handler = vi.fn();
  const unsubscribe = await onSelectToolFresh(handler);
  captured?.({ payload: "image.convert" });
  expect(handler).toHaveBeenCalledWith("image.convert");
  unsubscribe();
  expect(unlistenMock).toHaveBeenCalled();
});
```
(The dynamic `import("@tauri-apps/api/event")` inside `onSelectTool` plus `vi.doMock` + a fresh module import in the test is the simplest way to mock an event module only used by one function, without adding a static import that every other `wield.test.ts` case would then need to mock too. If a static top-level `import { listen } from "@tauri-apps/api/event"` proves easier to keep consistent with the rest of the file's mocking style, use that instead — implementer's call, not a design fork worth asking about.)

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git checkout -b feat/p6b-preferences
git add apps/wield/src/lib/wield.ts apps/wield/src/lib/wield.test.ts
git commit -m "feat(app-ui): hotkeyStatus, capabilities, onSelectTool wrappers"
```

---

## Task 2: `lib/settings.ts` — blur-to-hide

**Files:**
- Create: `apps/wield/src/lib/settings.ts`, `apps/wield/src/lib/settings.test.ts`

**Interfaces:**

```typescript
export function getBlurToHide(): boolean; // default true; reads localStorage, tolerant of missing/corrupt data
export function setBlurToHide(value: boolean): void;
```

- [x] **Step 1: Write the failing tests**

```typescript
import { beforeEach, expect, test } from "vitest";
import { getBlurToHide, setBlurToHide } from "./settings";

beforeEach(() => localStorage.clear());

test("defaults to true when unset", () => {
  expect(getBlurToHide()).toBe(true);
});

test("setBlurToHide persists and getBlurToHide reflects it", () => {
  setBlurToHide(false);
  expect(getBlurToHide()).toBe(false);
  setBlurToHide(true);
  expect(getBlurToHide()).toBe(true);
});

test("corrupt storage falls back to the default", () => {
  localStorage.setItem("wield.settings.blurToHide", "not json");
  expect(getBlurToHide()).toBe(true);
});
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/lib/settings.ts apps/wield/src/lib/settings.test.ts
git commit -m "feat(app-ui): blur-to-hide setting"
```

---

## Task 3: Window-label routing in `main.tsx`

**Files:**
- Modify: `apps/wield/src/main.tsx`

**Interfaces:**

```typescript
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles.css";
import App from "./App";
import Preferences from "./Preferences";

const label = getCurrentWindow().label;
const Root = label === "preferences" ? Preferences : App;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Root />
  </StrictMode>
);
```
`Preferences.tsx` doesn't exist until Task 4 — this task creates a minimal placeholder (`export default function Preferences() { return <div>Preferences</div>; }`) purely so the import resolves and `npm run check` stays green; Task 4 replaces it.

- [x] **Step 1: Implement the routing + placeholder `Preferences.tsx`**

(No new failing test here — this is wiring, not new logic. Verify manually: `npm run typecheck -w apps/wield` passes with the new import.)

- [x] **Step 2: Verify** — `export PATH="$HOME/.cargo/bin:$PATH" && npm run check` → PASS.
- [x] **Step 3: Commit**

```bash
git add apps/wield/src/main.tsx apps/wield/src/Preferences.tsx
git commit -m "feat(app-ui): route the preferences window to its own root component"
```

---

## Task 4: `HotkeySection`

**Files:**
- Create: `apps/wield/src/preferences/HotkeySection.tsx`, test, `HotkeySection.css`

**Interfaces:**

```typescript
export function HotkeySection(props: { status: HotkeyState | null }): JSX.Element;
```
`status === null` (still loading) → a quiet "Checking…" line. `{ state: "Pending" }` → same "Checking…" (P6b doesn't distinguish "still loading the component" from "backend hasn't finished binding yet" — both read the same to a user). `{ state: "Registered" }` → "Global shortcut: Super+W" (the literal trigger string isn't returned by the backend today — hardcode the known default `Super+W` per `wield-portal::global_shortcuts::PREFERRED_TRIGGER`, with a comment noting it should become dynamic if/when the backend ever reports the DE-confirmed trigger instead of just the preferred one). `{ state: "Unavailable", fallback_command }` → the reason text plus the command in a `--font-data` monospace chip with a "Copy" button (`navigator.clipboard.writeText`).

- [x] **Step 1: Write the failing tests**

```typescript
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { test, expect, vi } from "vitest";
import { HotkeySection } from "./HotkeySection";

test("shows a loading state before status arrives", () => {
  render(<HotkeySection status={null} />);
  expect(screen.getByText(/checking/i)).toBeInTheDocument();
});

test("shows the bound shortcut when registered", () => {
  render(<HotkeySection status={{ state: "Registered" }} />);
  expect(screen.getByText(/Super\+W/)).toBeInTheDocument();
});

test("shows the fallback command with a working copy button when unavailable", async () => {
  Object.assign(navigator, { clipboard: { writeText: vi.fn() } });
  render(<HotkeySection status={{ state: "Unavailable", fallback_command: "wield-app" }} />);
  expect(screen.getByText("wield-app")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: /copy/i }));
  expect(navigator.clipboard.writeText).toHaveBeenCalledWith("wield-app");
});
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/preferences/HotkeySection.tsx apps/wield/src/preferences/HotkeySection.test.tsx apps/wield/src/preferences/HotkeySection.css
git commit -m "feat(app-ui): Preferences hotkey section"
```

---

## Task 5: `PaletteBehaviorSection`

**Files:**
- Create: `apps/wield/src/preferences/PaletteBehaviorSection.tsx`, test

**Interfaces:**

```typescript
export function PaletteBehaviorSection(): JSX.Element;
```
Self-contained (reads/writes `lib/settings.ts` directly — no props needed, keeps `Preferences.tsx` from having to thread setting state through every section for a single checkbox). One checkbox: "Hide the palette when it loses focus", `checked={getBlurToHide()}` on mount via `useState(getBlurToHide)`, `onChange` calls `setBlurToHide(next)` and updates local state.

- [x] **Step 1: Write the failing tests**

```typescript
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, test, expect } from "vitest";
import { PaletteBehaviorSection } from "./PaletteBehaviorSection";
import { getBlurToHide } from "../lib/settings";

beforeEach(() => localStorage.clear());

test("checkbox reflects and updates the blur-to-hide setting", async () => {
  render(<PaletteBehaviorSection />);
  const checkbox = screen.getByRole("checkbox", { name: /hide the palette when it loses focus/i });
  expect(checkbox).toBeChecked();
  await userEvent.click(checkbox);
  expect(checkbox).not.toBeChecked();
  expect(getBlurToHide()).toBe(false);
});
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/preferences/PaletteBehaviorSection.tsx apps/wield/src/preferences/PaletteBehaviorSection.test.tsx apps/wield/src/preferences/*.css
git commit -m "feat(app-ui): Preferences palette-behaviour section"
```

---

## Task 6: `SystemStatusSection`

**Files:**
- Create: `apps/wield/src/preferences/SystemStatusSection.tsx`, test, css

**Interfaces:**

```typescript
export function SystemStatusSection(props: { report: CapabilitiesReport | null }): JSX.Element;
```
`report === null` → "Checking…" (same pattern as Task 4). Otherwise three small tables/lists:
- **Binaries** — one row per `Object.entries(report.binaries)`: name (monospace) + "Found" / "Not found".
- **Portals** — one row per `Object.entries(report.portals)`: interface name + `v${version}`. Empty → "No portals detected" (not an error state — just informational).
- **Tools** — one row per `report.tools`: title + "Available" or the `reason` text (dimmed, `--text-muted`), same visual language as P6a's `ToolRow` unavailable state for consistency.

- [x] **Step 1: Write the failing tests**

```typescript
import { render, screen } from "@testing-library/react";
import { test, expect } from "vitest";
import { SystemStatusSection } from "./SystemStatusSection";

const report = {
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
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/preferences/SystemStatusSection.tsx apps/wield/src/preferences/SystemStatusSection.test.tsx apps/wield/src/preferences/*.css
git commit -m "feat(app-ui): Preferences System Status section"
```

---

## Task 7: `Preferences.tsx` — assemble the three sections

**Files:**
- Replace: `apps/wield/src/Preferences.tsx` (the Task 3 placeholder)
- Create: `apps/wield/src/Preferences.test.tsx`

**Interfaces:**

```typescript
export default function Preferences(): JSX.Element;
```
On mount, calls `hotkeyStatus()` and `capabilities()` (each independently — one being slow shouldn't block the other), holds each in `useState<... | null>(null)`, renders a page with a title ("Wield Preferences") and the three sections in order: Hotkey, Palette behaviour, System Status. Reuses `.app-shell`-style framing from `styles.css` where sensible, but this window is a normal resizable window (not the frameless floating palette) — don't force the palette's `border-radius`/`box-shadow` onto it; a simple padded page is correct here.

- [x] **Step 1: Write the failing tests**

```typescript
import { render, screen } from "@testing-library/react";
import { beforeEach, test, expect, vi } from "vitest";

const hotkeyStatusMock = vi.fn();
const capabilitiesMock = vi.fn();
vi.mock("./lib/wield", async (importOriginal) => {
  const original = await importOriginal<typeof import("./lib/wield")>();
  return { ...original, hotkeyStatus: () => hotkeyStatusMock(), capabilities: () => capabilitiesMock() };
});

import Preferences from "./Preferences";

beforeEach(() => {
  hotkeyStatusMock.mockReset().mockResolvedValue({ state: "Registered" });
  capabilitiesMock.mockReset().mockResolvedValue({ binaries: {}, portals: {}, tools: [] });
});

test("renders all three sections once data loads", async () => {
  render(<Preferences />);
  expect(await screen.findByText(/Super\+W/)).toBeInTheDocument();
  expect(screen.getByRole("checkbox", { name: /hide the palette when it loses focus/i })).toBeInTheDocument();
  expect(screen.getByText(/system status/i)).toBeInTheDocument();
});
```

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/Preferences.tsx apps/wield/src/Preferences.test.tsx
git commit -m "feat(app-ui): assemble the Preferences window"
```

---

## Task 8: `App.tsx` — respect the blur-to-hide setting

**Files:**
- Modify: `apps/wield/src/App.tsx`, `apps/wield/src/App.test.tsx`

**Interfaces:** the existing blur `useEffect` (P6a) becomes:

```typescript
useEffect(() => {
  const onBlur = () => {
    if (getBlurToHide()) void hidePalette();
  };
  window.addEventListener("blur", onBlur);
  return () => window.removeEventListener("blur", onBlur);
}, []);
```
(`getBlurToHide` imported from `./lib/settings`.)

- [x] **Step 1: Write the failing test**

Append to `App.test.tsx`:

```typescript
test("blur does not hide the palette when the setting is off", async () => {
  const { setBlurToHide } = await import("./lib/settings");
  setBlurToHide(false);
  render(<App />);
  await screen.findByText("Pick a color");
  fireEvent(window, new Event("blur"));
  expect(hidePaletteMock).not.toHaveBeenCalled();
  setBlurToHide(true); // restore for other tests in the file
});
```
(Confirm the existing suite already has a passing-case blur test from P6a covering `hidePaletteMock` *is* called when the setting is on/default; if it doesn't, add one — this task's job is to prove **both** branches of the new conditional, not just the new one.)

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/App.tsx apps/wield/src/App.test.tsx
git commit -m "feat(app-ui): App respects the blur-to-hide setting"
```

---

## Task 9: `App.tsx` — tray tool selection

**Files:**
- Modify: `apps/wield/src/App.tsx`, `apps/wield/src/App.test.tsx`

**Interfaces:** a new `useEffect` subscribing via `onSelectTool` (Task 1):

```typescript
useEffect(() => {
  let unsubscribe: (() => void) | undefined;
  void onSelectTool((toolId) => {
    void listTools().then((tools) => {
      const tool = tools.find((t) => t.id === toolId);
      if (tool && tool.available) {
        dispatch(tool.args.length === 0 ? /* same action shape startRun uses */ : { type: "OPEN_FORM", tool });
        if (tool.args.length === 0) startRun(tool, {}, false);
      }
    });
  }).then((fn) => { unsubscribe = fn; });
  return () => unsubscribe?.();
}, [startRun]);
```
(Match this to whatever `startRun`/activation helper Task 12 of P6a actually named and how it's called from `SearchController`'s `onActivate` — **reuse that exact function**, don't duplicate the "no-args tool runs immediately, else open the form" branch a second time. Read the current `App.tsx` before writing this task's code to get the real helper name/signature; the snippet above is illustrative of the *behavior*, not a literal diff.)

- [x] **Step 1: Write the failing test**

Append to `App.test.tsx`:

```typescript
test("a tray tool-selection event opens the form for an argument tool", async () => {
  let deliver: ((toolId: string) => void) | undefined;
  vi.doMock("./lib/wield", async (importOriginal) => {
    const original = await importOriginal<typeof import("./lib/wield")>();
    return {
      ...original,
      listTools: listToolsMock,
      onSelectTool: async (handler: (id: string) => void) => {
        deliver = handler;
        return () => {};
      },
    };
  });
  const { default: FreshApp } = await import("./App");
  render(<FreshApp />);
  await screen.findByText("Pick a color");
  deliver?.("image.convert");
  expect(await screen.findByRole("heading", { name: "Convert image" })).toBeInTheDocument();
});
```
(If re-mocking `./lib/wield` mid-file via `vi.doMock` + a fresh dynamic `import("./App")` fights with the file's existing top-level `vi.mock`, the simpler alternative is adding `onSelectTool` to the file's *existing* top-level mock from the start — with a module-scoped `let selectToolHandler` the test can call directly — and skip the re-mock gymnastics entirely. Prefer the simpler alternative; the snippet shows the behavior to prove, not the only valid test structure.)

- [x] **Step 2: Run to verify failure** — `npm run test -w apps/wield`
- [x] **Step 3: Implement**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src/App.tsx apps/wield/src/App.test.tsx
git commit -m "feat(app-ui): wire tray tool selection into the palette"
```

---

## Task 10: Workspace green + P6b wrap-up

- [x] **Step 1: Full checks**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check
```
Expected: PASS. (The Rust side is untouched by this plan — these commands should be a no-op regression check, confirming that's actually true.)

- [x] **Step 2: Manual smoke (if a display/tray is available)**

Launch `wield-app`, open Preferences from the tray menu, confirm the hotkey/system-status sections show real data, toggle the blur setting, click a tool in the tray menu and confirm the palette opens to it. Report honestly if any of this isn't checkable in the execution environment (same standard as every prior plan).

- [x] **Step 3: Report to GEON**

```
agent-comms message post --to GEON --kind FYI --subject "P6b complete" --body "Preferences window now has real content: hotkey status + copyable fallback command, a blur-to-hide toggle, and a System Status page (binaries/portals/per-tool availability) from the existing hotkey_status/capabilities commands - no backend changes needed. Tray tool selection now opens the right tool in the palette. cargo test --workspace + clippy + npm run check green (Rust side untouched, confirmed no regressions). Manual UI smoke: <done | not possible, no display>. N commits, branch pushed."
```

- [x] **Step 4: Commit the plan + push**

```bash
git add docs/superpowers/plans/2026-09-11-wield-m1-p6b-preferences.md
git commit -m "docs: mark M1 P6b plan complete"
git push -u origin feat/p6b-preferences
```

GEON opens the PR, reviews, merges. **P7** (Flatpak packaging + CI) is last in M1 — written just-in-time.

---

## Self-Review

**Spec coverage:**

- §5 "Preferences … hotkey setup" → Task 4 (display; **not** rebinding — named out of scope) ✓
- §5 "launch-at-login" → explicitly deferred (GEON amendment) — needs new portal work, not silently dropped ✓
- §5 "palette behaviour (blur-to-hide, clear-query)" → blur-to-hide implemented (Tasks 2, 5, 8); **clear-query is not mentioned again after this line in the spec and has no other reference** — treating it as covered by "the query already clears naturally when Escape returns to a fresh search" (P6a's existing behavior) rather than inventing a second toggle with no spec detail behind it; noted here rather than silently expanded or silently dropped.
- §5 "default output directory, per-tool enable/disable" → both explicitly deferred (GEON amendment), named with the specific backend work each needs ✓
- §5 "System Status page — which portals and binaries were detected, and why any given tool is unavailable" → Task 6, using the existing `capabilities` command exactly as designed for this in P5a/P5b ✓
- §5 "the tray … route through it" (tool selection) → Tasks 1, 9 ✓

**Placeholder scan:** no TBD/TODO. The hardcoded `Super+W` display text in Task 4 is explicitly flagged as a known simplification (the backend doesn't report the DE-confirmed trigger, only whether binding succeeded) with a comment explaining why, not silently wrong.

**Type consistency:** `HotkeyState`/`CapabilitiesReport`/`ToolAvailability` (Task 1) match `apps/wield/src-tauri/src/{state,capabilities}.rs` exactly (verified against the real source while writing this plan — `HotkeyState` is the one internally-tagged exception among all the types this project's frontend has had to mirror so far, called out explicitly so it isn't handled with the P6a externally-tagged pattern by mistake).

**Ordering:** 1–2 (independent) → 3 (needs a `Preferences` placeholder, independent of 1/2's content) → 4–6 (each needs 1 or 2 for its data; independent of each other) → 7 (needs 3, 4, 5, 6) → 8 (needs 2) → 9 (needs 1, and P6a's existing activation helper) → 10. Consistent.

---

## Execution note

After P6b, every window has real content and every entry point (palette search, tray click, tray tool-select, the bound hotkey when available) reaches the right place. What's left in M1 is **P7**: Flatpak packaging (the manifest, bundled `ffmpeg`/`ImageMagick`/`pandoc`/`qpdf`/`tesseract`), AUR/`.deb`/`.rpm` secondary channels, and CI (the full test suite + the E2E smoke spec §8 describes — xvfb + dbus session + mock portal — which nothing before P7 has had the infrastructure to run). `1.0` is spec-gated on M1–M3, not M1 alone, so P7 finishes M1's slice but isn't the end of the project.

---

## Execution report (GEON, 2026-09-11)

Owner said "do it yourself" — GEON executed all 10 tasks directly on `feat/p6b-preferences`, no SUNIO involvement. TDD followed throughout (failing test confirmed red, then implemented, then green, every task).

- **No deviations from the plan's design.** `HotkeyState`/`CapabilitiesReport`/`ToolAvailability` types, the `localStorage` settings pattern, the window-label routing, and Task 9's tray-selection wiring all matched the plan's Interfaces sections as written — reused P6a's existing `activate(tool)` helper exactly, no duplicated branching logic.
- **One reusable test-infra note (not a plan bug):** `localStorage` is not available bare in this project's Vitest/jsdom setup — every test touching it needs the `Object.defineProperty(window, "localStorage", {...})` polyfill already established by P6a's `recents.test.ts`. `settings.test.ts` follows that pattern.
- **Task 9's test** used the plan's own documented "simpler alternative" (a module-scoped `selectToolHandler` captured by the file's existing top-level `vi.mock("./lib/wield", ...)`) rather than the `vi.doMock` + dynamic re-import sketch, exactly as the plan flagged as preferred.
- **Full verification, all green:** `cargo fmt --all` (no diff — Rust side untouched, confirmed), `cargo clippy --workspace --all-targets -- -D warnings` (clean), `cargo test --workspace` (all `ok`, 0 failed, same pre-existing ignored/manual-only tests as before), `npm run check` (eslint clean, tsc clean, vitest 20 files / 70 tests passed, up from P6a's 51).
- **Manual smoke — partially checkable, reported honestly (same standard as P6a's hotkey attempt):** rebuilt the frontend dist + debug binary fresh (not the stale mid-session instance) and launched `wield-app` in this environment. Confirmed via D-Bus/logs: starts headless (`wield shell ready (headless)`), owns the single-instance name `io.github.DhanushSantosh.Wield`, and registers as a real `StatusNotifierItem` (`busctl` shows it in `org.kde.StatusNotifierWatcher`'s `RegisteredStatusNotifierItems`, `Title` = `"wield-app"`, icon = the current placeholder DeskCrafter desk illustration, matching the known not-yet-replaced icon). Located the actual tray icon in the sandboxed desktop's panel and attempted to click through to the Preferences window and a tray tool-selection, but pixel-precise clicking in this virtualized desktop proved unreliable (repeated misclicks landed on the OS control center instead of the tray dropdown) — did not force it further. Preferences window content, the blur toggle, and tray-driven tool activation are NOT visually confirmed live; they are covered by the automated Preferences.test.tsx/App.test.tsx suites instead (14 + 3 tests, including two integration-style tests that drive the real `onSelectTool`→`listTools`→`activate` path end-to-end).
- **N = 9 commits** on `feat/p6b-preferences` (Tasks 1–9 one each, plus this plan-completion commit). Branch pushed to `origin/feat/p6b-preferences`.
