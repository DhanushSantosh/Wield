# Wield M1 · Plan P1 — Foundation & Rename — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the DeskCrafter-named working tree into the Wield project — rename the npm and Cargo workspaces, scaffold the four Wield crates as compiling stubs, reduce the desktop app to a placeholder, remove DeskCrafter-specific docs and cruft, and keep `npm run check` green.

**Architecture:** A mechanical foundation slice with no new runtime behavior. The repository at `/home/dhanush/Projects/Wield` was freshly `git init`-ed on 2026-09-10 with the DeskCrafter source tree and no history. This plan renames identifiers, deletes the launcher domain code (not carried over — `launcher.doctor` is rebuilt from scratch at M4), scaffolds `wield-core` / `wield-portal` / `wield-tools` / `wield-cli`, transforms `apps/desktop` → `apps/wield` with a single `app_info` command, and replaces the launcher UI with a placeholder.

**Tech Stack:** Rust (2021 edition, Cargo workspace), Tauri 2.9, React 19 + Vite 7 + Vitest 4, npm workspaces, Node ≥ 20.

**Spec:** `docs/superpowers/specs/2026-09-10-wield-design.md`

---

## Status: COMPLETE — 2026-09-10 (commit `4671e9a`, pushed to origin/master)

Executed by GEON in-session per the amendment below (clean additive scaffold, no
per-task commits). Result verified: `cargo build --workspace` ✓, `cargo test
--workspace` ✓ (3 unit tests), `cargo run -p wield-cli` → `wield 0.1.0` ✓,
`npm run check` ✓, `cargo fmt --check` ✓, no DeskCrafter strings. The task-by-task
steps below are retained for reference; the delivered tree is the single first
commit, not the diff series they describe. Next: P2 (wield-core descriptor +
executor), plan written just-in-time.

---

## GEON amendment — 2026-09-10 (owner decisions; overrides conflicting steps below)

The owner resolved the three open items. This amendment is authoritative where it
conflicts with the task steps.

1. **Clean scaffold, zero DeskCrafter in history.** There is **no "Initial import"
   commit** and **no per-task commits**. **Task 1 is skipped entirely.** The worker
   makes **no git commits and no git operations at all** — the first commit is made
   by GEON after review, once the tree is clean.
   - Build the Wield tree **additively**: the DeskCrafter working tree (`crates/core/`,
     `apps/desktop/`, `apps/site/`, `ONBOARDING.md`, `.claude/worktrees/`) stays on
     disk **only as reference material** while you work, and must **not** appear in
     the final tree. Do not `git mv` / `git rm` — just create the `wield-*` files and,
     as the last step, delete the DeskCrafter directories from the working tree.
   - Every `- [ ] Step … Commit` and every `git add` / `git commit` / `git mv` /
     `git rm` in Tasks 2–7 is a **no-op** — perform the file changes it describes,
     skip the git command.
2. **`apps/site/` is dropped from P1.** Do not carry it into the Wield tree at all.
   - Root `package.json` `workspaces` = `["apps/wield"]` only.
   - Drop the `dev:site`, `dev:site:preview`, `build:site`, `dev:side-by-side`,
     `dev:app:preview`, and `fix:generated` scripts and the `concurrently` /
     `normalize-next-env` machinery. Keep: `dev`, `dev:app`, `build:app`,
     `build:app:ui`, `lint`, `typecheck`, `test`, `check`, `cargo:test`.
   - `lint` / `typecheck` / `check` run against `apps/wield` + cargo only.
   - `README.md`: drop the `apps/site` bullet.
   - A Wield marketing site is a separate later plan.
3. **`apps/wield/src-tauri/gen/` is gitignored.** Add `apps/*/src-tauri/gen/` to
   `.gitignore`. Do not stage it.
4. **`.claude/` is gitignored.** Append `.claude/` to `.gitignore` (runtime state +
   the stale worktree copy should never be tracked).
5. **Config source:** you may read the existing `apps/desktop/**` and `crates/core/**`
   config (Vite, tsconfig, ESLint, `vite.config.ts`, `tauri.conf.json`, `build.rs`,
   `src/main.tsx`, `capabilities/`, `package.json` deps, `Cargo.toml`) as boilerplate
   and adapt it into `apps/wield` / `crates/wield-core`, but **write every file fresh
   and de-branded** — no DeskCrafter identifiers anywhere in the result.
6. **`Cargo.lock`** stays tracked (app/bin workspace — correct). Regenerate it clean.
7. **App identifier is `io.github.DhanushSantosh.Wield`** (owner decision 2026-09-10 —
   `wield.dev` / `wield.app` are registered to third parties, so the `dev.wield.Wield`
   form has no domain backing and Flathub would reject it; the GitHub-namespace id is
   always accepted). Replace **every** `dev.wield.Wield` in the steps below with
   `io.github.DhanushSantosh.Wield` — `tauri.conf.json` `identifier`, and anywhere
   else it appears. (The D-Bus single-instance name is P5, not P1.)

**Revised end-to-end flow:** worker executes P1 (no commits) → reports to GEON →
GEON + owner review the working tree → GEON makes the **single first commit** →
GEON does the **first push** to `origin` → P2.

---

## Global Constraints

- **Repo root:** `/home/dhanush/Projects/Wield` (branch `master`, `origin` = `github.com/DhanushSantosh/Wield.git`). All paths below are relative to this root.
- **Name:** project is `Wield`. Binary `wield`. Crates `wield-core`, `wield-portal`, `wield-tools`, `wield-cli`. Tauri app crate `wield-app`, lib name `wield_app_lib`. App identifier `io.github.DhanushSantosh.Wield` (see GEON amendment §7). npm root package `wield`, app package `@wield/app`.
- **Rust edition:** 2021, from `[workspace.package]`.
- **Node/npm floor:** Node ≥ 20.0.0, npm ≥ 10.0.0.
- **No launcher functionality** in M1–M3. P1 removes all launcher code from the tree; there is **no archive branch and no snapshot** (owner's decision, 2026-09-10).
- **No push.** The first push to `origin` is coordinated separately by GEON / the owner. This plan makes local commits only.
- **`npm run check`** (lint + typecheck + test, both apps + cargo) must pass at the end of every task that touches buildable code.

### First-commit shape (unresolved — see spec §12 Q2)

This plan is written for the **diff-friendly** variant: **Task 1** makes an "Initial import" commit of the current tree, and Tasks 2–7 transform it as reviewable commits. If the owner instead wants **zero DeskCrafter content in history**, skip Task 1, drop every `git commit` step in Tasks 2–7, and let GEON make a single first commit of the finished P1 tree. Confirm with GEON before starting.

---

## File Structure

**Deleted (not carried over — recoverable only from this plan's "Initial import" commit, if made):**

- `crates/core/src/{desktop_entry,error,icons,inspectors,launcher_library,platform,safety,types}.rs`
- `crates/core/src/tools/` (entire directory)
- `crates/core/tests/{launcher_library,tool_registry}.rs`
- `apps/desktop/src/components/`, `apps/desktop/src/hooks/`, `apps/desktop/src/lib/` (entire directories)
- `ONBOARDING.md` (DeskCrafter onboarding doc)
- `.claude/worktrees/` (stale orphaned-worktree copy of the old tree that travelled with the move)

**Renamed:**

- `crates/core/` → `crates/wield-core/`
- `apps/desktop/` → `apps/wield/`

**Created:**

- `crates/wield-core/src/lib.rs` — stub: crate version helper + module doc (real model in P2).
- `crates/wield-portal/{Cargo.toml,src/lib.rs}` — stub crate (real `ashpd` wrapper in P3).
- `crates/wield-tools/{Cargo.toml,src/lib.rs}` — stub crate (real descriptors in P4).
- `crates/wield-cli/{Cargo.toml,src/main.rs}` — stub binary `wield` printing name + version (real CLI in P4).
- `apps/wield/src/App.tsx` — placeholder screen.
- `apps/wield/src/App.test.tsx` — smoke test.

**Modified:**

- `Cargo.toml` (root), `package.json` (root), `scripts/run-cargo.js`, `README.md`
- `crates/wield-core/Cargo.toml`
- `apps/wield/package.json`, `apps/wield/index.html`
- `apps/wield/src-tauri/{Cargo.toml,src/main.rs,src/lib.rs,tauri.conf.json,capabilities/default.json}`

**Deferred to later plans / out of P1 scope:** the `docs/` product-doc rewrite (architecture, backend-contracts, packaging, product-scope, tool-specs, frontend-theme, development, testing) — P1 only *removes* nothing from `docs/` except by explicit step below and *adds* the superpowers spec + this plan. `apps/site/` content rewrite is a separate plan; P1 leaves it building.

---

## Task 1: Initial import commit

**Files:** all currently-untracked files at repo root.

**Interfaces:**
- Produces: commit 1 (`master`) — the DeskCrafter source tree plus the Wield spec and this plan under `docs/superpowers/`, as the base for the P1 transformation diffs.

- [ ] **Step 1: Confirm the git state**

Run: `git -C /home/dhanush/Projects/Wield status --short && git -C /home/dhanush/Projects/Wield log --oneline`
Expected: many `??` untracked entries; `log` errors with "does not have any commits yet".

- [ ] **Step 2: Verify `.gitignore` covers build output and node_modules**

Run: `cat /home/dhanush/Projects/Wield/.gitignore`
Expected: contains `target/`, `node_modules/`, `dist/`, `.next/`. If `.claude/` is **not** listed, add it now (the stale worktree copy inside it is deleted in Task 4, and live `.claude/` runtime state should not be tracked) — append a line `.claude/` to `.gitignore`.

- [ ] **Step 3: Stage and commit**

```bash
cd /home/dhanush/Projects/Wield
git add -A
git commit -m "Initial import"
```

- [ ] **Step 4: Verify**

Run: `git log --stat --oneline -1 | head -40`
Expected: one commit `Initial import` listing the source tree.

---

## Task 2: Repurpose `crates/core` → `crates/wield-core` stub

**Files:**
- Rename: `crates/core/` → `crates/wield-core/`
- Modify: `crates/wield-core/Cargo.toml`, `Cargo.toml` (root)
- Replace: `crates/wield-core/src/lib.rs`
- Delete: `crates/wield-core/src/{desktop_entry,error,icons,inspectors,launcher_library,platform,safety,types}.rs`, `crates/wield-core/src/tools/`, `crates/wield-core/tests/`

**Interfaces:**
- Produces: crate `wield-core` exposing `pub fn version() -> &'static str` returning `env!("CARGO_PKG_VERSION")`. P2 replaces `lib.rs` wholesale; nothing depends on its internals yet.

- [ ] **Step 1: Move the crate directory**

```bash
cd /home/dhanush/Projects/Wield
git mv crates/core crates/wield-core
```

- [ ] **Step 2: Delete the launcher domain modules and tests**

```bash
git rm crates/wield-core/src/desktop_entry.rs crates/wield-core/src/error.rs \
  crates/wield-core/src/icons.rs crates/wield-core/src/inspectors.rs \
  crates/wield-core/src/launcher_library.rs crates/wield-core/src/platform.rs \
  crates/wield-core/src/safety.rs crates/wield-core/src/types.rs
git rm -r crates/wield-core/src/tools crates/wield-core/tests
```

- [ ] **Step 3: Replace `crates/wield-core/src/lib.rs`**

```rust
//! `wield-core` — descriptor model, executor, and tool registry for Wield.
//!
//! Stub in plan P1. The descriptor/executor model lands in P2.

/// The `wield-core` crate version, from Cargo.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!version().is_empty());
    }
}
```

- [ ] **Step 4: Rewrite `crates/wield-core/Cargo.toml`**

```toml
[package]
name = "wield-core"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true
```

(The `[dev-dependencies] tempfile` line is dropped; P2 re-adds what it needs.)

- [ ] **Step 5: Update the root `Cargo.toml`**

```toml
[workspace]
members = [
  "crates/wield-core",
  "apps/desktop/src-tauri"
]
default-members = ["crates/wield-core"]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT"
repository = "https://github.com/DhanushSantosh/Wield"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
uuid = { version = "1", features = ["v4", "serde"] }
```

(`apps/desktop/src-tauri` stays until Task 4; the Task 3 crates are added there.)

- [ ] **Step 6: Verify `wield-core` builds and tests pass**

Run: `cargo test -p wield-core`
Expected: PASS (`version_is_non_empty`).

- [ ] **Step 7: Verify the Tauri app now fails to build (expected)**

Run: `cargo check -p deskcrafter-desktop`
Expected: FAIL — unresolved imports from `deskcrafter_core`. Fixed in Task 4; do not fix here.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor: repurpose crates/core into wield-core stub"
```

---

## Task 3: Scaffold `wield-portal`, `wield-tools`, `wield-cli`

**Files:**
- Create: `crates/wield-portal/{Cargo.toml,src/lib.rs}`, `crates/wield-tools/{Cargo.toml,src/lib.rs}`, `crates/wield-cli/{Cargo.toml,src/main.rs}`
- Modify: `Cargo.toml` (root)

**Interfaces:**
- Consumes: `wield-core` — `wield_core::version()`.
- Produces:
  - `wield-portal` with `pub fn probe_stub() -> bool` returning `true` (P3 replaces).
  - `wield-tools` with `pub fn descriptor_count() -> usize` returning `0` (P4 replaces).
  - binary crate `wield-cli` producing an executable `wield` that prints `wield <version>` and exits 0.

- [ ] **Step 1: Create `wield-portal`**

`crates/wield-portal/Cargo.toml`:

```toml
[package]
name = "wield-portal"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
wield-core = { path = "../wield-core" }
```

`crates/wield-portal/src/lib.rs`:

```rust
//! `wield-portal` — XDG Desktop Portal access and startup capability probe.
//!
//! Stub in plan P1. The `ashpd` wrapper and capability probe land in P3.

/// Placeholder for the startup capability probe. Always `true` until P3.
pub fn probe_stub() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn probe_stub_is_true() {
        assert!(super::probe_stub());
    }
}
```

- [ ] **Step 2: Create `wield-tools`**

`crates/wield-tools/Cargo.toml`:

```toml
[package]
name = "wield-tools"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
wield-core = { path = "../wield-core" }
```

`crates/wield-tools/src/lib.rs`:

```rust
//! `wield-tools` — built-in tool descriptors and native tool implementations.
//!
//! Stub in plan P1. The `color.pick` and `image.convert` descriptors land in P4.

/// Number of built-in descriptors. Zero until P4.
pub fn descriptor_count() -> usize {
    0
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_descriptors_yet() {
        assert_eq!(super::descriptor_count(), 0);
    }
}
```

- [ ] **Step 3: Create `wield-cli`**

`crates/wield-cli/Cargo.toml`:

```toml
[package]
name = "wield-cli"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true

[[bin]]
name = "wield"
path = "src/main.rs"

[dependencies]
wield-core = { path = "../wield-core" }
```

`crates/wield-cli/src/main.rs`:

```rust
//! `wield` — the Wield command-line surface.
//!
//! Stub in plan P1. The registry-generated command surface lands in P4.

fn main() {
    println!("wield {}", wield_core::version());
}
```

- [ ] **Step 4: Register all four crates in the root workspace**

`Cargo.toml` (root) `members` / `default-members`:

```toml
members = [
  "crates/wield-core",
  "crates/wield-portal",
  "crates/wield-tools",
  "crates/wield-cli",
  "apps/desktop/src-tauri"
]
default-members = ["crates/wield-core", "crates/wield-portal", "crates/wield-tools", "crates/wield-cli"]
```

- [ ] **Step 5: Build and test the new crates**

Run: `cargo test -p wield-portal -p wield-tools -p wield-cli`
Expected: PASS (`probe_stub_is_true`, `no_descriptors_yet`; `wield-cli` has no tests — fine).

- [ ] **Step 6: Verify the `wield` binary runs**

Run: `cargo run -p wield-cli`
Expected: prints `wield 0.1.0`, exit 0.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: scaffold wield-portal, wield-tools, wield-cli stub crates"
```

---

## Task 4: Transform `apps/desktop` → `apps/wield`, reduce the Tauri backend

**Files:**
- Rename: `apps/desktop/` → `apps/wield/`
- Delete: `.claude/worktrees/` (stale orphaned copy)
- Modify: `apps/wield/src-tauri/{Cargo.toml,src/main.rs,tauri.conf.json,capabilities/default.json}`, `Cargo.toml` (root)
- Replace: `apps/wield/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `wield-core`.
- Produces: Tauri command `app_info() -> AppInfo` where `AppInfo { name: String, version: String }` (`name` = `"Wield"`, `version` = `wield_core::version()`). The frontend (Task 5) calls it via `invoke("app_info")`.

- [ ] **Step 1: Move the app directory and delete the stale worktree copy**

```bash
cd /home/dhanush/Projects/Wield
git mv apps/desktop apps/wield
git rm -r --ignore-unmatch .claude/worktrees
rm -rf .claude/worktrees
```

- [ ] **Step 2: Rewrite `apps/wield/src-tauri/Cargo.toml`**

```toml
[package]
name = "wield-app"
version = "0.1.0"
description = "Wield — a portal-native Linux utility hub"
edition.workspace = true
license.workspace = true
repository.workspace = true

[lib]
name = "wield_app_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2.5.3", features = [] }

[dependencies]
wield-core = { path = "../../../crates/wield-core" }
serde.workspace = true
serde_json.workspace = true
tauri = { version = "2.9.5", features = [] }
```

- [ ] **Step 3: Rewrite `apps/wield/src-tauri/src/main.rs`**

```rust
fn main() {
    wield_app_lib::run();
}
```

- [ ] **Step 4: Replace `apps/wield/src-tauri/src/lib.rs`**

```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Wield".to_string(),
        version: wield_core::version().to_string(),
    }
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running Wield");
}
```

- [ ] **Step 5: Rewrite `apps/wield/src-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Wield",
  "version": "0.1.0",
  "identifier": "io.github.DhanushSantosh.Wield",
  "build": {
    "beforeDevCommand": "npm run dev:ui -- --host 127.0.0.1 --port 1420",
    "devUrl": "http://127.0.0.1:1420",
    "beforeBuildCommand": "npm run build:ui",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      { "title": "Wield", "width": 1280, "height": 820, "minWidth": 980, "minHeight": 640 }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": [],
    "category": "Utility",
    "shortDescription": "Portal-native Linux utility hub",
    "longDescription": "A command palette and tray for quick Linux actions and file conversions."
  }
}
```

(Window kept for now; the hidden palette window is P5. `bundle.targets` empty until the Flatpak pipeline in P7.)

- [ ] **Step 6: Update `capabilities/default.json`**

Change `"description"` to `"Default permissions for the main Wield window"`. Leave `"windows"` and `"permissions"` unchanged.

- [ ] **Step 7: Point the root workspace at the renamed app crate**

`Cargo.toml` (root): change `"apps/desktop/src-tauri"` → `"apps/wield/src-tauri"` in `members`.

- [ ] **Step 8: Regenerate Tauri schemas and verify the backend compiles**

```bash
rm -rf apps/wield/src-tauri/gen
cargo check -p wield-app
```
Expected: PASS. `gen/schemas` is regenerated by `tauri-build`. (If `apps/wield/src-tauri/gen/` was tracked, `git add -A` in Step 10 picks up the regenerated files; confirm `gen/` is not in `.gitignore` — the current repo tracks it.)

- [ ] **Step 9: Confirm no lingering `deskcrafter` identifiers in the app crate**

```bash
grep -rn -i "deskcrafter" apps/wield/src-tauri/src apps/wield/src-tauri/Cargo.toml apps/wield/src-tauri/tauri.conf.json
```
Expected: no matches.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "refactor: transform apps/desktop into apps/wield, reduce backend to app_info"
```

---

## Task 5: Strip the desktop frontend to a placeholder

**Files:**
- Delete: `apps/wield/src/components/`, `apps/wield/src/hooks/`, `apps/wield/src/lib/`
- Replace: `apps/wield/src/App.tsx`
- Create: `apps/wield/src/App.test.tsx`, possibly `apps/wield/vitest.setup.ts`
- Modify: `apps/wield/index.html`, `apps/wield/package.json`, possibly `apps/wield/vitest.config.ts`

**Interfaces:**
- Consumes: `app_info` Tauri command (Task 4).
- Produces: nothing consumed later (P6 replaces `App.tsx` with the palette shell).

- [ ] **Step 1: Delete the launcher UI**

```bash
cd /home/dhanush/Projects/Wield
git rm -r apps/wield/src/components apps/wield/src/hooks apps/wield/src/lib
```

- [ ] **Step 2: Write the placeholder `apps/wield/src/App.tsx`**

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface AppInfo {
  name: string;
  version: string;
}

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info")
      .then(setInfo)
      .catch(() => setInfo(null));
  }, []);

  return (
    <main className="app-shell">
      <h1>{info ? info.name : "Wield"}</h1>
      <p>Portal-native Linux utility hub.</p>
      {info && <p className="version">v{info.version}</p>}
    </main>
  );
}
```

- [ ] **Step 3: Write the smoke test `apps/wield/src/App.test.tsx`**

```tsx
import { render, screen } from "@testing-library/react";
import { vi } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({ name: "Wield", version: "0.1.0" }),
}));

test("renders the Wield heading", async () => {
  render(<App />);
  expect(await screen.findByRole("heading", { name: "Wield" })).toBeInTheDocument();
});
```

- [ ] **Step 4: Run the test to see whether jsdom matchers are set up**

Run: `npm run test -w apps/wield`
Expected: either PASS, or FAIL on `toBeInTheDocument` being undefined.

- [ ] **Step 5: If Step 4 failed on `toBeInTheDocument`, add the matcher setup**

Create `apps/wield/vitest.setup.ts`:

```ts
import "@testing-library/jest-dom/vitest";
```

Add `"@testing-library/jest-dom": "^6"` to `apps/wield/package.json` devDependencies. Update `apps/wield/vitest.config.ts`:

```ts
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./vitest.setup.ts"],
  },
});
```

Run `npm install` from the repo root.

- [ ] **Step 6: Run the test to verify it passes**

Run: `npm run test -w apps/wield`
Expected: PASS.

- [ ] **Step 7: Update `index.html` and `package.json`**

`apps/wield/index.html`: `<title>Wield</title>` (and any other DeskCrafter text).
`apps/wield/package.json`: `"name": "@wield/app"`.

- [ ] **Step 8: Confirm nothing imports the deleted modules**

```bash
grep -rn "components/\|hooks/\|lib/api\|lib/launcher\|lib/types" apps/wield/src
```
Expected: no matches.

- [ ] **Step 9: Typecheck and test**

Run: `npm run typecheck -w apps/wield && npm run test -w apps/wield`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "refactor: replace launcher UI with Wield placeholder screen"
```

---

## Task 6: Rename root tooling and remove DeskCrafter docs/cruft

**Files:**
- Modify: `package.json` (root), `scripts/run-cargo.js`, `README.md`
- Delete: `ONBOARDING.md`

**Interfaces:**
- Produces: npm scripts `dev`, `dev:app`, `dev:app:preview`, `dev:side-by-side`, `build:app`, `build:app:ui`, `dev:site`, `dev:site:preview`, `build:site`, `lint`, `typecheck`, `test`, `check`, `cargo:test`, `fix:generated` — all pointing at `apps/wield` / `apps/site`.

- [ ] **Step 1: Rewrite the root `package.json`**

```json
{
  "name": "wield",
  "private": true,
  "workspaces": ["apps/wield", "apps/site"],
  "scripts": {
    "dev": "npm run dev:app",
    "dev:app": "npm run dev -w apps/wield",
    "dev:app:preview": "npm run dev:ui -w apps/wield -- --host 127.0.0.1 --port 1421",
    "dev:side-by-side": "concurrently -n app,site -c blue,magenta \"npm run dev:app:preview\" \"npm run dev:site:preview\"",
    "build:app": "npm run build -w apps/wield",
    "build:app:ui": "npm run build:ui -w apps/wield",
    "dev:site": "npm run dev -w apps/site",
    "dev:site:preview": "npm run dev -w apps/site -- -p 3001",
    "build:site": "npm run build -w apps/site",
    "lint": "npm run lint -w apps/wield && npm run lint -w apps/site",
    "typecheck": "npm run typecheck -w apps/wield && npm run typecheck -w apps/site",
    "test": "npm run test -w apps/wield && node scripts/run-cargo.js test",
    "check": "npm run lint && npm run typecheck && npm run test",
    "cargo:test": "node scripts/run-cargo.js test",
    "fix:generated": "node scripts/normalize-next-env.js apps/site"
  },
  "engines": { "node": ">=20.0.0", "npm": ">=10.0.0" },
  "devDependencies": {
    "@tauri-apps/cli": "^2.9.2",
    "concurrently": "^9.2.1"
  }
}
```

- [ ] **Step 2: Update `scripts/run-cargo.js` strings**

Replace `"Install Rustup so DeskCrafter can run Cargo checks on Windows."` → `"Install Rustup so Wield can run Cargo checks on Windows."`, and the temp file name `"deskcrafter-run-cargo.cmd"` → `"wield-run-cargo.cmd"`.

- [ ] **Step 3: Rewrite `README.md`**

```markdown
# Wield

Wield is a portal-native Linux utility hub — a global-hotkey command palette and
tray that front a suite of quick actions (screen capture tools) and file/data
converters. It is built on XDG Desktop Portals so it works across desktop
environments without per-DE code.

- `apps/wield`: Tauri + Vite + React desktop shell.
- `apps/site`: Wield product website.
- `crates/wield-core`: descriptor model, executor, and tool registry.
- `crates/wield-portal`: XDG Desktop Portal access and capability probe.
- `crates/wield-tools`: built-in tool descriptors and native tools.
- `crates/wield-cli`: the `wield` command-line surface.

## Development

    npm install
    npm run check          # lint + typecheck + tests (both apps + cargo)
    npm run dev:app        # run the Tauri shell
    npm run dev:site       # run the product site

## License

MIT License. See [LICENSE](LICENSE).
```

- [ ] **Step 4: Delete `ONBOARDING.md`**

```bash
git rm ONBOARDING.md
```

- [ ] **Step 5: Reinstall workspaces and run the full check**

```bash
npm install
npm run check
```
Expected: PASS — lint, typecheck, vitest for `apps/wield` + `apps/site`, then `cargo test` across the four `wield-*` crates and `wield-app`.

- [ ] **Step 6: Scan for stale build-critical references**

```bash
grep -rn -i "deskcrafter\|apps/desktop" \
  package.json Cargo.toml scripts/ apps/wield/ crates/ README.md \
  --include='*.json' --include='*.toml' --include='*.js' --include='*.ts' --include='*.tsx' --include='*.rs' --include='*.md'
```
Expected: no matches. (`docs/` product docs and `apps/site/` content still contain DeskCrafter strings — intentionally out of P1 scope; the docs rewrite and site rewrite are separate plans. `LICENSE` keeps its `Copyright (c) 2025 DhanushSantosh` line.)

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "chore: rename npm workspace and scripts to Wield; drop DeskCrafter onboarding"
```

---

## Task 7: Verify the full workspace and record P1 completion

**Files:**
- Modify: this plan file (check the boxes)

**Interfaces:**
- Consumes: Tasks 2–6.
- Produces: a green baseline for P2.

- [ ] **Step 1: Clean build**

```bash
cd /home/dhanush/Projects/Wield
cargo clean && cargo build --workspace
```
Expected: PASS — `wield-core`, `wield-portal`, `wield-tools`, `wield-cli`, `wield-app` all compile.

- [ ] **Step 2: Full check**

Run: `npm run check`
Expected: PASS.

- [ ] **Step 3: Confirm the `wield` binary**

Run: `cargo run -p wield-cli`
Expected: prints `wield 0.1.0`.
(Tauri shell: `npm run dev:app` launches the placeholder window on a machine with Tauri's Linux dev libraries — not a P1 gate.)

- [ ] **Step 4: Post P1 completion to the coordination channel**

Report to GEON: P1 landed on `master` at `/home/dhanush/Projects/Wield`, `npm run check` green, workspace renamed, four `wield-*` crates scaffolded, app reduced to placeholder, DeskCrafter docs/cruft removed. Not pushed. Ready for P2 (wield-core descriptor + executor).

- [ ] **Step 5: Commit the checked-off plan**

```bash
git add docs/superpowers/plans/2026-09-10-wield-m1-p1-foundation-rename.md
git commit -m "docs: mark M1 P1 plan complete"
```

---

## Self-Review

**Spec coverage (P1 scope — spec §10 "Starting point & initial restructure"):**

- `crates/core` launcher domain → deleted; `crates/wield-core` stub → Tasks 2, 3 ✓
- New crates `wield-core` / `wield-portal` / `wield-tools` / `wield-cli` → Tasks 2, 3 ✓
- `apps/desktop` → `apps/wield`, identifier `io.github.DhanushSantosh.Wield`, backend to placeholder → Task 4 ✓; "hidden palette + tray" is **P5**, not P1 (Task 4 keeps the window) ✓
- `apps/site` left building → Tasks 6 Step 5 verifies `npm run check` includes site ✓; content rewrite is a separate plan ✓
- `package.json`, npm scripts, root `Cargo.toml`, `README.md` renamed → Tasks 2, 4, 6 ✓
- `ONBOARDING.md`, stale `.claude/worktrees/*` removed → Tasks 4, 6 ✓
- `docs/*` product-doc rewrite → **separate plan** (spec §10 / §11); P1 does not touch them beyond adding the superpowers spec + this plan ✓
- Wield uses fresh `~/.config/wield` etc. → no persisted state in P1 ✓

**Placeholder scan:** No "TBD"/"TODO" in task steps. Stub functions (`probe_stub`, `descriptor_count`, `version`) are documented scaffolding replaced in named later plans.

**Type consistency:** `wield_core::version() -> &'static str` defined Task 2, consumed Tasks 3, 4. `AppInfo { name, version }` defined Task 4, consumed Task 5 (`App.tsx` + mock). Consistent.

**Open items for GEON / owner before P1 implementation:**

1. **First-commit shape** (spec §12 Q2). This plan assumes Task 1 makes an "Initial import" commit and Tasks 2–7 are reviewable diffs. If the owner wants zero DeskCrafter content in git history, drop Task 1 and all per-task `git commit` steps; GEON makes one first commit of the finished tree. **Confirm before starting.**
2. **`apps/wield/src-tauri/gen/` tracking.** The current repo tracks the generated Tauri schema dir. P1 regenerates it (Task 4 Step 8). If GEON prefers it gitignored, add `apps/*/src-tauri/gen/` to `.gitignore` in Task 1 Step 2 and `git rm -r --cached` it in Task 4.
3. **`Cargo.lock`.** Deleting crates and adding stubs churns `Cargo.lock`; it is committed with each task. Confirm the workspace `Cargo.lock` should stay tracked (it should — this is an app/bin workspace).

---

## Execution note

P2–P7 plans are written just-in-time after each predecessor lands, because each depends on interface decisions made during the previous plan's implementation (descriptor struct shape from P2 drives P3/P4; Tauri command signatures from P5 drive P6). The spec's §6 milestone list and §3–§9 are the contract those plans implement.
