# Wield M1 · Plan P7 — CI + Flatpak packaging pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the two pieces of infrastructure M1 has been missing since P1: a GitHub Actions CI workflow that gates every push/PR with the full test suite plus a real end-to-end smoke test, and a working Flatpak packaging pipeline (manifest, AppStream metainfo, a release workflow that builds and lints it on tags) that produces an installable app — culminating in opening the actual Flathub submission PR.

**Architecture:** Two independent GitHub Actions workflows. `.github/workflows/ci.yml` runs on every push/PR to `master`: fast unit/lint job (fmt, clippy, `cargo test --workspace`, `npm run check`) plus a slower E2E job (xvfb + a fresh D-Bus session, real `magick` on `$PATH`) that drives the shell over its existing single-instance D-Bus `RunTool` method exactly the way a second CLI invocation already does. `.github/workflows/release.yml` runs only on `v*` tags: builds the Flatpak via `flatpak-builder` inside the runner, lints it with `flatpak-builder-lint`, uploads a single-file bundle. The Flatpak manifest builds `wield-app` offline (vendored cargo + npm sources, the same two-generator pattern Tauri's own Flatpak guide documents) and extracts the binary/desktop-file/icons from a `tauri build -b deb` output rather than hand-rolling Tauri's bundling logic a second time.

**Tech Stack:** GitHub Actions, `flatpak-builder` + `flatpak-builder-lint`, `flatpak-cargo-generator.py` / `flatpak-node-generator` (both from `flatpak/flatpak-builder-tools`), AppStream 0.16 MetaInfo XML. No new Rust or npm dependencies.

**Spec:** `docs/superpowers/specs/2026-09-10-wield-design.md` §8 "Testing strategy" (E2E smoke) and §9 "Packaging & distribution" (Flatpak, CI/release).

---

## GEON amendment — 2026-09-11

Four scope decisions, made with the owner before writing this plan:

1. **Flatpak build validated in CI only.** `flatpak`/`flatpak-builder` are not installed in this dev sandbox and installing them needs `sudo pacman -S` plus a multi-GB runtime download. Every step in this plan that touches `flatpak-builder` itself runs in GitHub Actions; local work produces and validates the manifest, AppStream file, and the two vendored-sources JSON files (both generator scripts run standalone, no `flatpak-builder` needed for that part) without ever invoking `flatpak-builder` in this sandbox. **No `sudo` commands anywhere in this plan.**
2. **AUR / `.deb` / `.rpm` secondary channels deferred entirely.** The spec's Milestones section names only "Flatpak packaging pipeline" for M1; §9's secondary channels become a later, separate packaging plan. Not touched here.
3. **Icon stays the placeholder DeskCrafter illustration** (`assets/icon.png`, `apps/wield/src-tauri/icons/icon.png`) for this plan. Flagged prominently in `docs/packaging.md` and in the Flathub submission itself (Task 8) as a known pre-review blocker — Flathub review will very likely reject a submission whose icon doesn't represent the app, so the submission is expected to come back with that request. Swapping the file needs no pipeline changes.
4. **The five bundled converter binaries (ImageMagick, pandoc, qpdf+ghostscript, tesseract+eng, and the `ffmpeg-full` runtime extension) are OUT of scope for this plan.** Checked `flathub/shared-modules` directly — no ready-made module exists for any of them; each needs its own from-scratch build-module research (source URLs, versions, checksums, build system, dependency chain). Real per-tool research, not a checkbox. This plan's Flatpak manifest builds `wield-app` alone against the plain `org.gnome.Platform` runtime. Consequence: **inside the Flatpak, `image.convert` reports `Unavailable` (no `magick` on `$PATH` in the sandbox) — only `color.pick` (Portal-based, no bundled binary needed) works.** This is honest, not broken: the existing probe-at-startup availability model already handles "tool present but binary missing" correctly, and it is exactly what the spec's own `Unavailable { reason, fix }` outcome exists for. A follow-up plan (**P7-tools**, JIT like every prior plan) adds the five build modules; nothing here blocks it.
5. **Flathub submission is IN scope (Task 8)** but is a hard-gated step: opening a PR against a third-party public repository (`flathub/flathub`) is a "publish public content" action. **Whoever executes Task 8 MUST stop and get an explicit, real-time go-ahead in chat immediately before forking `flathub/flathub` or opening the PR** — the scope approval already given for this plan does not substitute for that per-action confirmation. If no fresh confirmation is available, stop at Task 8 and hand the prepared submission (fork instructions + manifest + metainfo, all already committed in-repo) back for a human to submit, or for a later session to submit once confirmed.
6. **Mechanical-correction rule (as every prior plan):** exact generator-script invocations, Tauri's `-b deb` output layout (filenames of the extracted `.desktop`/icon/binary), and GitHub Actions runner package names may drift from what's documented below — check what actually happens and adapt; note it in the Task 10 report. In particular: `flatpak-node-generator` has a known compatibility issue with some npm lockfile shapes (a real user hit a "v4 lockfile" error and worked around it by switching package managers — do **not** do that here if hit; this repo's `package-lock.json` is `lockfileVersion: 3`, the documented-supported shape, so this is flagged as a low-probability risk to watch for, not a fallback to reach for).
7. **`cargo` is not on the default `PATH`.** `export PATH="$HOME/.cargo/bin:$PATH"` before every local cargo invocation.
8. **`npm run check`** green at the end of every task that touches buildable code.

---

## Global Constraints

- **Repo root:** `/home/dhanush/Projects/Wield` (branch `feat/p7-packaging-ci` off `master`).
- **App-id everywhere:** `io.github.DhanushSantosh.Wield` — the Flatpak manifest filename, the AppStream metainfo filename, the `finish-args` `--own-name`, and the D-Bus object path all use it verbatim; Flathub requires the manifest/metainfo/desktop-file basenames to match the app-id exactly.
- **`finish-args` (locked in the design spec, do not add to it without a scope conversation):**
  ```
  --socket=wayland
  --socket=fallback-x11
  --share=ipc
  --device=dri
  --talk-name=org.freedesktop.portal.Desktop
  --talk-name=org.kde.StatusNotifierWatcher
  --own-name=io.github.DhanushSantosh.Wield
  ```
  No `--filesystem=host`.
- **Runtime:** `org.gnome.Platform`/`org.gnome.Sdk`, version `50` (current on Flathub as of 2026-09-11 — verify this is still current when the task runs; Flathub rejects submissions against an EOL runtime).

---

## File Structure

**Created:**

| File | Responsibility |
| --- | --- |
| `.github/workflows/ci.yml` | Fast job (fmt/clippy/test/npm check) on every push+PR to `master`; slow job (E2E smoke under xvfb+dbus) alongside it |
| `apps/wield/src-tauri/tests/launch.rs` (modified) | New `#[ignore]` E2E smoke test: real `RunTool` D-Bus call against `image.convert` |
| `packaging/flatpak/io.github.DhanushSantosh.Wield.yml` | The Flatpak manifest |
| `packaging/flatpak/io.github.DhanushSantosh.Wield.metainfo.xml` | AppStream MetaInfo |
| `packaging/flatpak/cargo-sources.json` | Vendored Rust crate sources (generated, committed — Flatpak builds are offline) |
| `packaging/flatpak/node-sources.json` | Vendored npm package sources (generated, committed) |
| `.github/workflows/release.yml` | Builds + lints the Flatpak on `v*` tags |
| `docs/packaging.md` | Documents the pipeline, generator commands used, every deferred item named |

---

## Task 1: CI — fast test-suite job

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:** none (infrastructure only) — no code changes to test.

- [x] **Step 1: Write the workflow**

```yaml
name: CI

on:
  push:
    branches: [master]
  pull_request:
    branches: [master]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Tauri Linux build dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            build-essential \
            curl \
            wget \
            file \
            libxdo-dev \
            libssl-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Install Node
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm

      - run: npm ci
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
      - run: npm run check
```

- [x] **Step 2: Verify — push and watch it run**

```bash
git checkout -b feat/p7-packaging-ci
git add .github/workflows/ci.yml
git commit -m "ci: add fast test-suite workflow"
git push -u origin feat/p7-packaging-ci
gh run watch --exit-status
```
Expected: the run appears (triggered by the push, since GitHub Actions picks up new workflow files immediately once pushed on any branch — this doesn't need a PR open) and finishes green. If it fails, read the log (`gh run view --log-failed`), fix, push again — treat any failure here as a real bug to fix, not a plan defect, since every command in the workflow already passes locally on this exact worktree.

- [x] **Step 3: Commit** (already done as part of Step 2 — this step exists only if Step 2 required fix-up commits; squash or leave as separate commits, whichever is cleaner)

---

## Task 2: E2E smoke test — real `RunTool` over D-Bus

**Files:**
- Modify: `apps/wield/src-tauri/tests/launch.rs`

**Interfaces:**
- Consumes: `instance::BUS_NAME` = `"io.github.DhanushSantosh.Wield"`, `instance::OBJECT_PATH` = `"/io/github/DhanushSantosh/Wield"` (both `pub const` in `apps/wield/src-tauri/src/instance.rs`) — the D-Bus object exposes `async fn run_tool(id: &str, args_json: &str) -> String`, returning `serde_json::to_string(&ToolOutcome)`. `wield_core::ToolOutcome::File { path: PathBuf }` is the success variant, externally tagged as `{"File":{"path":"..."}}`.
- Uses `assets/icon.png` (already committed, a valid PNG) as the fixture input — no new binary test asset needed.

- [x] **Step 1: Write the failing test**

Append to `apps/wield/src-tauri/tests/launch.rs`:

```rust
#[test]
#[ignore = "needs a display + session bus + a real `magick` on PATH; run with --ignored"]
fn run_tool_converts_a_real_fixture_end_to_end() {
    if std::env::var("WAYLAND_DISPLAY").is_err() && std::env::var("DISPLAY").is_err() {
        eprintln!("skipping: no display");
        return;
    }
    if std::process::Command::new("magick")
        .arg("-version")
        .output()
        .is_err()
    {
        eprintln!("skipping: magick unavailable");
        return;
    }

    let output_dir = tempfile::tempdir().unwrap();
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir
        .join("../../../assets/icon.png")
        .canonicalize()
        .expect("assets/icon.png should exist at the repo root");

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_wield-app"))
        .env("XDG_STATE_HOME", output_dir.path())
        .spawn()
        .unwrap();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut owned = false;
    while std::time::Instant::now() < deadline {
        owned = std::process::Command::new("busctl")
            .args(["--user", "list"])
            .output()
            .map(|output| {
                String::from_utf8_lossy(&output.stdout).contains("io.github.DhanushSantosh.Wield")
            })
            .unwrap_or(false);
        if owned {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    assert!(owned, "shell should own the D-Bus name before RunTool is callable");

    let outcome_json = wield_run_tool_over_dbus(
        "image.convert",
        &serde_json::json!({
            "input": fixture.to_string_lossy(),
            "format": "webp",
        })
        .to_string(),
    );

    let _ = child.kill();
    let _ = child.wait();

    let outcome: serde_json::Value =
        serde_json::from_str(&outcome_json).expect("RunTool should return JSON");
    let path = outcome["File"]["path"]
        .as_str()
        .unwrap_or_else(|| panic!("expected a File outcome, got: {outcome_json}"));
    assert!(
        std::path::Path::new(path).exists(),
        "converted output file should exist at {path}"
    );
}

/// Calls `RunTool` on the primary Wield instance over a session-bus D-Bus
/// proxy — the same call the CLI's second-instance relay path makes
/// (`apps/wield/src-tauri/src/instance.rs::relay`), driven here directly
/// instead of through a second `wield-app` process.
fn wield_run_tool_over_dbus(id: &str, args_json: &str) -> String {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let connection = zbus::Connection::session().await.unwrap();
        let proxy = zbus::Proxy::new(
            &connection,
            "io.github.DhanushSantosh.Wield",
            "/io/github/DhanushSantosh/Wield",
            "io.github.DhanushSantosh.Wield",
        )
        .await
        .unwrap();
        proxy
            .call::<_, _, String>("RunTool", &(id, args_json))
            .await
            .unwrap()
    })
}
```

- [x] **Step 2: Run to verify it fails or is skipped honestly**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p wield-app --test launch -- --ignored run_tool_converts_a_real_fixture_end_to_end
```
Expected: in this sandbox (no `magick` on `$PATH` — confirmed earlier in this session that ImageMagick isn't bundled/installed here), the test prints `skipping: magick unavailable` and passes trivially. That's correct, honest behavior for this environment, not a false green — the real assertion only runs where `magick` is present (this repo's own CI, once Task 3 installs it). If `magick` *is* present locally, it should genuinely run and pass since `image.convert`'s Command path is already implemented and tested (P4).

- [x] **Step 3: Commit**

```bash
git add apps/wield/src-tauri/tests/launch.rs
git commit -m "test(e2e): RunTool converts a real fixture over D-Bus end to end"
```

---

## Task 3: CI — wire the E2E smoke test into the workflow

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:** none new — runs Task 2's test with `magick` actually present, so it exercises the real assertion path this time.

- [x] **Step 1: Add the E2E job**

Append a second job to `.github/workflows/ci.yml` (same file, after `test:`):

```yaml
  e2e-smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Tauri Linux build dependencies + E2E prerequisites
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            build-essential \
            curl \
            wget \
            file \
            libxdo-dev \
            libssl-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            xvfb \
            dbus-x11 \
            imagemagick

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Run E2E smoke under a virtual display + fresh D-Bus session
        run: |
          export PATH="$HOME/.cargo/bin:$PATH"
          xvfb-run -a dbus-run-session -- \
            cargo test -p wield-app --test launch -- --ignored --test-threads=1
```

`--test-threads=1`: both `--ignored` tests in `launch.rs` spawn a real `wield-app` process and race for the same single-instance D-Bus name — running them serially avoids the two tests fighting over it.

- [x] **Step 2: Verify — push and watch both jobs**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run the E2E smoke test under xvfb + a fresh D-Bus session"
git push
gh run watch --exit-status
```
Expected: both `test` and `e2e-smoke` jobs green, and this time `run_tool_converts_a_real_fixture_end_to_end` should actually run its real assertions (`magick` is now on `$PATH` via `apt-get install imagemagick`) rather than skip — check the job log for the `skipping: magick unavailable` line's *absence* to confirm this, not just a green checkmark (a skip and a real pass both look green).

- [x] **Step 3: Commit** (folded into Step 2 above if no fixes were needed)

---

## Task 4: Flatpak manifest — app-only build

**Files:**
- Create: `packaging/flatpak/io.github.DhanushSantosh.Wield.yml`
- Create: `packaging/flatpak/cargo-sources.json`
- Create: `packaging/flatpak/node-sources.json`

**Interfaces:** none (packaging config, no Rust/TS code) — this task's "test" is generating real, well-formed vendored-sources files and a manifest that at least parses as valid YAML; the actual `flatpak-builder` build is verified in Task 6 (CI-only, per the GEON amendment).

- [x] **Step 1: Generate the vendored cargo sources**

```bash
cd /home/dhanush/Projects/Wield
curl -sL -o /tmp/flatpak-cargo-generator.py \
  https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
python3 /tmp/flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
```
Expected: `packaging/flatpak/cargo-sources.json` exists and is well-formed JSON (`python3 -c "import json; json.load(open('packaging/flatpak/cargo-sources.json'))"` exits 0) with a nonzero number of entries (one per vendored crate).

- [x] **Step 2: Generate the vendored npm sources**

```bash
pip install --user flatpak-node-generator
python3 -m flatpak_node_generator --no-requests-cache -o packaging/flatpak/node-sources.json npm package-lock.json
```
Expected: same well-formed-JSON check. **Mechanical-correction note:** if `pip install flatpak-node-generator` fails to resolve (package renamed/moved), fall back to cloning `flatpak/flatpak-builder-tools` and running `node/flatpak-node-generator.py` directly — both are documented entry points to the same tool.

- [x] **Step 3: Write the manifest**

```yaml
app-id: io.github.DhanushSantosh.Wield
runtime: org.gnome.Platform
runtime-version: '50'
sdk: org.gnome.Sdk
sdk-extensions:
  - org.freedesktop.Sdk.Extension.node20
  - org.freedesktop.Sdk.Extension.rust-stable
command: wield-app
finish-args:
  - --socket=wayland
  - --socket=fallback-x11
  - --share=ipc
  - --device=dri
  - --talk-name=org.freedesktop.portal.Desktop
  - --talk-name=org.kde.StatusNotifierWatcher
  - --own-name=io.github.DhanushSantosh.Wield

modules:
  - name: wield
    buildsystem: simple
    build-options:
      append-path: /usr/lib/sdk/node20/bin:/usr/lib/sdk/rust-stable/bin
      env:
        CARGO_HOME: /run/build/wield/cargo
        npm_config_cache: /run/build/wield/npm-cache
    build-commands:
      - . /usr/lib/sdk/node20/enable.sh
      - . /usr/lib/sdk/rust-stable/enable.sh
      - npm ci --offline --cache /run/build/wield/npm-cache
      - npm run tauri build -w apps/wield -- -b deb --offline
      - mkdir -p /tmp/deb-extract
      - ar x apps/wield/src-tauri/target/release/bundle/deb/*.deb --output=/tmp/deb-extract
      - tar xf /tmp/deb-extract/data.tar.* -C /tmp/deb-extract
      - install -Dm755 /tmp/deb-extract/usr/bin/wield-app /app/bin/wield-app
      - |
        install -Dm644 /tmp/deb-extract/usr/share/applications/*.desktop \
          /app/share/applications/io.github.DhanushSantosh.Wield.desktop
      - |
        for size in 32x32 128x128 256x256; do
          src="/tmp/deb-extract/usr/share/icons/hicolor/$size/apps"
          if [ -d "$src" ]; then
            install -Dm644 "$src"/*.png \
              "/app/share/icons/hicolor/$size/apps/io.github.DhanushSantosh.Wield.png"
          fi
        done
      - install -Dm644 packaging/flatpak/io.github.DhanushSantosh.Wield.metainfo.xml \
          /app/share/metainfo/io.github.DhanushSantosh.Wield.metainfo.xml
    sources:
      - type: dir
        path: ../..
      - cargo-sources.json
      - node-sources.json
```

**Mechanical-correction notes (real unknowns, resolved at execution time, not guessed here):**
- The exact filenames Tauri's `-b deb` bundler produces under `.../bundle/deb/*.deb`, and the exact names of the `.desktop`/icon files inside it, are confirmed by actually running this in CI (Task 6) — the glob patterns above are written to tolerate the real names rather than hard-code a guess.
- `npm run tauri build -w apps/wield -- -b deb --offline`: confirm this is the right invocation for this repo's actual root `package.json` script wiring (whether `tauri` needs `-w apps/wield` from the root, or must run from inside `apps/wield`) — adjust if the real command differs; note the correction in the Task 10 report.
- `--offline` on `npm run tauri build`: Tauri's build step itself doesn't take npm flags directly — this may need to become `npm run build -w apps/wield --offline` (the Vite build) followed by a separate `cargo tauri build --offline` or equivalent inside `apps/wield/src-tauri`. Resolve against whatever actually runs in this repo's `apps/wield/package.json` `build` script and adjust; the goal (build the frontend + Tauri binary fully offline from the vendored sources) is fixed, the exact command isn't.

- [x] **Step 4: Commit**

```bash
git add packaging/flatpak/
git commit -m "feat(packaging): Flatpak manifest, vendored cargo/npm sources"
```

---

## Task 5: AppStream MetaInfo

**Files:**
- Create: `packaging/flatpak/io.github.DhanushSantosh.Wield.metainfo.xml`

**Interfaces:** consumed by Task 4's manifest (`install -Dm644 ... metainfo.xml ...`) and by Flathub review (Task 8).

- [x] **Step 1: Write it**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>io.github.DhanushSantosh.Wield</id>
  <name>Wield</name>
  <summary>A portal-native command palette and tray for Linux</summary>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>MIT</project_license>
  <description>
    <p>
      Wield is a global-hotkey command palette and tray for quick actions and
      file conversions on Linux. It runs entirely through the XDG Desktop
      Portal APIs — no host filesystem access, no elevated permissions — and
      keeps every tool invocation local, with no telemetry and no network
      calls.
    </p>
    <p>
      This release bundles the shell, the tray, the global-shortcut portal
      integration, and two tools: a Portal-based color picker and a
      Command-based image converter. Additional converters and their bundled
      binaries ship in a follow-up release.
    </p>
  </description>
  <launchable type="desktop-id">io.github.DhanushSantosh.Wield.desktop</launchable>
  <url type="homepage">https://github.com/DhanushSantosh/Wield</url>
  <url type="bugtracker">https://github.com/DhanushSantosh/Wield/issues</url>
  <content_rating type="oars-1.1" />
  <categories>
    <category>Utility</category>
  </categories>
  <releases>
    <release version="0.1.0" date="2026-09-11">
      <description>
        <p>Initial architecture-slice release: palette, tray, global shortcut,
        color picker, image converter.</p>
      </description>
    </release>
  </releases>
</component>
```

No `<screenshots>` block — deliberately omitted rather than filled with a placeholder image (AppStream doesn't require one; Flathub review strongly prefers at least one, so this is expected to come back as a review request, tracked alongside the icon in `docs/packaging.md`, Task 7).

- [x] **Step 2: Validate it's well-formed XML**

```bash
python3 -c "import xml.dom.minidom; xml.dom.minidom.parse('packaging/flatpak/io.github.DhanushSantosh.Wield.metainfo.xml')"
```
Expected: exits 0, no exception.

- [x] **Step 3: Commit**

```bash
git add packaging/flatpak/io.github.DhanushSantosh.Wield.metainfo.xml
git commit -m "feat(packaging): AppStream MetaInfo"
```

---

## Task 6: CI — Flatpak build + lint on tags

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:** builds Task 4's manifest; this is where the manifest's real correctness (Task 4's mechanical-correction notes) actually gets resolved against reality.

- [x] **Step 1: Write the workflow**

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  flatpak:
    runs-on: ubuntu-latest
    container:
      image: bilelmoussaoui/flatpak-github-actions:gnome-47
      options: --privileged
    steps:
      - uses: actions/checkout@v4

      - uses: flatpak/flatpak-github-actions/flatpak-builder@v6
        with:
          bundle: wield.flatpak
          manifest-path: packaging/flatpak/io.github.DhanushSantosh.Wield.yml
          cache-key: flatpak-builder-${{ github.sha }}

      - name: Lint the manifest and the built repo
        run: |
          flatpak install -y --noninteractive flathub org.flatpak.Builder
          flatpak run org.flatpak.Builder --lint-manifest \
            packaging/flatpak/io.github.DhanushSantosh.Wield.yml || true
          flatpak run org.flatpak.Builder --lint-appstream \
            packaging/flatpak/io.github.DhanushSantosh.Wield.metainfo.xml || true
```

**Mechanical-correction note:** the `bilelmoussaoui/flatpak-github-actions:gnome-47` container image tag is a well-known, actively maintained community image for exactly this job (the `flatpak/flatpak-github-actions` action's own docs recommend it) — but it pins GNOME 47, one version behind this plan's runtime-version 50. Check at execution time whether a `gnome-50`-tagged image exists; if not, either the manifest's `runtime-version` or the container tag needs to move to whichever version the image actually provides — note which way it was resolved in the Task 10 report. The `--lint-manifest`/`--lint-appstream` flags on `org.flatpak.Builder` are the actual `flatpak-builder-lint` tool's interface as of writing; if the invocation has changed, adapt and note it. `|| true` on the lint steps: a first-pass lint against a deliberately minimal, no-screenshots submission (Task 5) is expected to report real, known warnings (icon, screenshots) — the step should still print them (for Task 8's submission-readiness read), not fail the job over already-known, already-tracked gaps.

- [x] **Step 2: Verify — push a disposable pre-release tag and watch it**

```bash
git tag v0.1.0-p7-smoke
git push origin v0.1.0-p7-smoke
gh run watch --exit-status
```
Expected: the `flatpak` job runs (this will take a while — downloading the `org.gnome.Platform`/`Sdk` runtime plus the `node20`/`rust-stable` SDK extensions the first time, on the order of 10-20 minutes) and finishes green, with the lint steps' output showing the expected icon/screenshot warnings and nothing else. Fix any real build failures (per Task 4's mechanical-correction notes) and re-push the tag (delete + re-create it) until it's genuinely green. Once confirmed, delete the disposable tag:

```bash
git push origin :refs/tags/v0.1.0-p7-smoke
git tag -d v0.1.0-p7-smoke
```

- [x] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: build and lint the Flatpak on version tags"
```

---

## Task 7: `docs/packaging.md`

**Files:**
- Create: `docs/packaging.md`

**Interfaces:** none — documentation only.

- [x] **Step 1: Write it**

```markdown
# Packaging Wield

## Flatpak (primary channel)

- Manifest: `packaging/flatpak/io.github.DhanushSantosh.Wield.yml`
- AppStream MetaInfo: `packaging/flatpak/io.github.DhanushSantosh.Wield.metainfo.xml`
- Vendored sources (regenerate after any `Cargo.lock`/`package-lock.json` change):
  ```bash
  python3 flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
  python3 -m flatpak_node_generator --no-requests-cache -o packaging/flatpak/node-sources.json npm package-lock.json
  ```
  (`flatpak-cargo-generator.py` from `flatpak/flatpak-builder-tools`'s `cargo/` directory; `flatpak_node_generator` via `pip install flatpak-node-generator`.)
- CI builds and lints the Flatpak on every `v*` tag push (`.github/workflows/release.yml`). Local builds need `flatpak` + `flatpak-builder` installed (not present in the primary dev sandbox as of this writing — validated in CI only, see P7's plan doc for why).

### Known gaps, named and tracked (not silently dropped)

- **Icon.** Still the placeholder DeskCrafter desk illustration (`assets/icon.png`). Flathub review is expected to reject on this — swap the file and re-run the vendored-source regeneration + a tag push once a real icon exists; no other pipeline change needed.
- **No AppStream screenshots.** Same story — add a `<screenshots>` block to the metainfo once real UI screenshots exist.
- **Bundled converter binaries deferred.** ImageMagick, pandoc, qpdf+ghostscript, tesseract+eng, and the `ffmpeg-full` runtime extension are NOT bundled. Inside the Flatpak, only `color.pick` (Portal-based) is available; `image.convert` reports `Unavailable` (no `magick` on `$PATH` in the sandbox). A follow-up plan, **P7-tools**, adds the five build modules — each needs its own real packaging research (no ready-made Flathub shared module exists for any of them, checked directly).
- **AUR / `.deb` / `.rpm` secondary channels.** Not started. Later, separate plan per the design spec §9.

## Flathub submission status

See `docs/superpowers/plans/2026-09-11-wield-m1-p7-packaging-ci.md` Task 8 for the submission record (PR link once opened, or the reason it wasn't if execution stopped at the confirmation gate).
```

- [x] **Step 2: Commit**

```bash
git add docs/packaging.md
git commit -m "docs: packaging pipeline overview"
```

---

## Task 8: Flathub submission

**Files:** none in this repo (the submission lives in a fork of `flathub/flathub`); this repo's `docs/packaging.md` gets a one-line update once the PR exists (Step 4).

> **STOP — hard gate.** Before any command in this task, get an explicit, real-time go-ahead in chat for forking `flathub/flathub` and opening a PR against it. The scope approval that put "submit to Flathub" in this plan is a design-level decision, not the per-action confirmation this specific outward-facing publish still needs (per the operating rules governing this session: opening a PR on a third-party public repository is a "publish public content" action). If that confirmation isn't available when this task is reached, stop here — everything up through Task 7 stands on its own as a complete, working local pipeline regardless of whether this task runs.

- [x] **Step 1: Fork `flathub/flathub`**

```bash
gh repo fork flathub/flathub --clone=false
```

- [x] **Step 2: Add the submission on a new branch**

Flathub's own repo layout convention is one directory per app-id at the root:

```bash
git clone https://github.com/DhanushSantosh/flathub.git /tmp/flathub-fork
cd /tmp/flathub-fork
git checkout -b add-io.github.DhanushSantosh.Wield
mkdir io.github.DhanushSantosh.Wield
cp /home/dhanush/Projects/Wield/packaging/flatpak/io.github.DhanushSantosh.Wield.yml \
   io.github.DhanushSantosh.Wield/
git add io.github.DhanushSantosh.Wield/
git commit -m "Add io.github.DhanushSantosh.Wield"
git push -u origin add-io.github.DhanushSantosh.Wield
```

- [x] **Step 3: Open the PR**

```bash
gh pr create --repo flathub/flathub \
  --base master \
  --head DhanushSantosh:add-io.github.DhanushSantosh.Wield \
  --title "Add io.github.DhanushSantosh.Wield" \
  --body "New submission: Wield, a portal-native command palette and tray for Linux. Source: https://github.com/DhanushSantosh/Wield. Known gap flagged up front: the icon is a placeholder pending a real design asset (tracked in the upstream repo's docs/packaging.md) — expect that to come up in review."
```

- [x] **Step 4: Record the outcome**

Update `docs/packaging.md`'s "Flathub submission status" section with the PR URL (or, if the confirmation gate stopped execution, a note that Task 8 is prepared but not yet run, with a pointer to this plan).

```bash
cd /home/dhanush/Projects/Wield
git add docs/packaging.md
git commit -m "docs: record the Flathub submission PR"
```

---

## Task 9: Branch protection — require the CI status check

**Files:** none (GitHub repo setting, not a file in this repo).

> This changes repo governance (branch protection is a standing/persistent configuration) — confirm with the owner before running Step 1, same category of action as Task 8's gate, though lower-stakes (reversible, affects only this repo).

- [x] **Step 1: Add the required status check**

```bash
gh api repos/DhanushSantosh/Wield/branches/master/protection \
  --method PUT \
  --input - <<'EOF'
{
  "required_status_checks": {
    "strict": true,
    "contexts": ["test"]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": {
    "required_approving_review_count": 0,
    "dismiss_stale_reviews": true
  },
  "required_conversation_resolution": true,
  "restrictions": null,
  "allow_force_pushes": false,
  "allow_deletions": false
}
EOF
```
This preserves every existing branch-protection setting from P1 (0 approvals, `enforce_admins=false`, no force-push, no deletion, required conversation resolution) and adds `ci.yml`'s `test` job as a required check. `contexts: ["test"]` — not `"e2e-smoke"` — deliberately: the E2E job takes minutes and touches a real D-Bus session, higher flake surface than the fast job; gating merges on it is a call worth revisiting once it's proven stable over a few real PRs, not made unilaterally here.

- [x] **Step 2: Verify**

```bash
gh api repos/DhanushSantosh/Wield/branches/master/protection/required_status_checks
```
Expected: `contexts` includes `"test"`.

- [x] **Step 3: Commit** — nothing to commit (a GitHub API-side setting); this step is a no-op, kept for consistency with the plan's task numbering.

---

## Task 10: Workspace green + P7 wrap-up

- [x] **Step 1: Full local checks** (everything not gated behind CI-only Flatpak tooling)

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check
python3 -c "import xml.dom.minidom; xml.dom.minidom.parse('packaging/flatpak/io.github.DhanushSantosh.Wield.metainfo.xml')"
python3 -c "import json; json.load(open('packaging/flatpak/cargo-sources.json')); json.load(open('packaging/flatpak/node-sources.json'))"
```
Expected: PASS. (Same as every prior plan's wrap-up gate — the Rust/frontend code itself is untouched by this plan, confirming no regression, plus the two new file-validity checks this plan actually adds content for.)

- [x] **Step 2: Confirm both CI workflows are green on the branch**

```bash
gh run list --branch feat/p7-packaging-ci --limit 5
```
Expected: the most recent `CI` run (Task 3) is green. The `Release` run (Task 6) was already verified and its disposable tag cleaned up — confirm no stray `v0.1.0-p7-smoke` tag remains (`git tag -l 'v0.1.0-p7-smoke'` empty, both locally and via `git ls-remote --tags origin`).

- [x] **Step 3: Report to GEON** (only if this plan was dispatched to SUNIO — skip if GEON is executing this directly)

```
agent-comms message post --to GEON --kind FYI --subject "P7 complete" --body "CI (.github/workflows/ci.yml) gates every push/PR: fmt+clippy+cargo test+npm check, plus a real E2E smoke test (RunTool over D-Bus, image.convert on a real fixture, under xvfb+dbus-run-session). Release workflow builds+lints the Flatpak on version tags. Flatpak manifest builds wield-app only (color.pick works inside the sandbox; image.convert reports Unavailable - no bundled magick yet, deferred to a P7-tools follow-up, named not silently dropped). Flathub submission: <PR link | stopped at the confirmation gate, prepared but not opened>. Branch protection now requires the CI 'test' job. N commits, branch pushed."
```

- [x] **Step 4: Mark the plan complete, commit, push**

```bash
git add docs/superpowers/plans/2026-09-11-wield-m1-p7-packaging-ci.md
git commit -m "docs: mark M1 P7 plan complete"
git push
```

Then open the PR (if GEON is executing this directly, matching the P1/P4/P5b/P6b pattern) or hand off to GEON for review (if SUNIO executed it).

**M1 is complete once this PR merges** — every plan P1 through P7 shipped. `1.0` is still gated on M2 (converter suite) and M3 (capture) per the design spec, but the architecture slice — shell, portal path, command path, packaging pipeline — is done end-to-end.

---

## Self-Review

**Spec coverage:**

- §8 "E2E smoke (CI: xvfb + dbus session + mock portal)" → Tasks 2–3. Note: the spec's own phrase "mock portal" doesn't apply to what Task 2 actually tests — `image.convert` is a `Command` tool with no portal dependency at all, so the real E2E path needs no portal mock; `color.pick` (the one Portal-based tool) is *not* exercised end-to-end by this plan's E2E test, staying consistent with the spec's own "Out of scope: real compositor portal behavior in CI (manual matrix instead)" line just below it.
- §9 "Primary: Flatpak / Flathub", the `finish-args` block, and the app-id → Tasks 4–6, 8, verbatim from the spec.
- §9 "CI / release: GitHub Actions on tag: run full test suite + E2E smoke → build Flatpak" → Tasks 1, 3, 6 cover this exactly, split into a push/PR-gating fast+E2E pair and a tag-triggered packaging job (a refinement over "everything on tag," decided with the owner — the fast tests need to gate every PR, not just releases, for the required-status-check branch protection the spec's own §-open-items already anticipated).
- §9 "Secondary" (AUR/.deb/.rpm) → explicitly deferred, named in the GEON amendment and `docs/packaging.md`, not silently dropped.
- §11 "Repo artifacts to produce" → `packaging/flatpak/*` ✓, `.github/workflows/*` ✓, `docs/packaging.md` ✓ (partially — the fuller `docs/` rewrite to the descriptor/executor model the spec also names there stays a separate, later plan, as §10's own table already says).

**Placeholder scan:** no TBD/TODO. Every genuinely unresolved detail (exact `-b deb` output filenames, the lint container's GNOME version tag, `flatpak-node-generator`'s exact npm entry point) is flagged as a named, bounded mechanical-correction note with a concrete resolution path — the established pattern from every prior plan in this project — not a vague placeholder.

**Type consistency:** Task 2's test uses `instance::BUS_NAME`/`OBJECT_PATH` and the `run_tool(id, args_json) -> String` signature exactly as defined in `apps/wield/src-tauri/src/instance.rs` (read directly while writing this plan, not guessed) and the same `{"input": ..., "format": ...}` args shape already proven correct by `commands.rs`'s own `run_tool_runs_a_command_tool_against_a_stub` test.

**Ordering:** 1 (independent) → 2 (independent, needs only existing code) → 3 (needs 2's test to exist) → 4–5 (independent of 1–3, needs nothing but the repo's existing `Cargo.lock`/`package-lock.json`) → 6 (needs 4, 5) → 7 (needs 4–6's real outcomes to document accurately) → 8 (needs 4–7, hard-gated) → 9 (needs 1, 3 green at least once) → 10. Consistent.

---

## Execution report

SUNIO executed Tasks 1–7 on `feat/p7-packaging-ci`, iterating through the Task 6 "push a disposable tag and watch it" verification loop for real (13 commits, several `fix(...)` commits from that loop), then hit its usage limit mid-iteration — the Release workflow was still red at that point (4 straight failed attempts on `v0.1.0-p7-smoke`), no report posted to GEON's inbox. Owner asked GEON to continue; GEON picked up from the branch's actual state (not from any status report — none existed) by reading the real CI/Release run logs directly.

**SUNIO's real, correct deviations from the plan's literal text** (all legitimate, none reverted):
- Built via `npm run build -w apps/wield -- --no-bundle --ci` + direct binary/desktop-file/icon install, instead of the plan's `tauri build -b deb` + extract-from-.deb approach — Tauri's Debian bundler probes for a host AppIndicator package the GNOME SDK doesn't provide, even though Wield's tray support loads dynamically at runtime. Hand-authored the `.desktop` file and a 512×512 icon export instead of extracting them. Cleaner than the plan's guess.
- `ci.yml` triggers on `push:` (any branch) + `workflow_dispatch`, not just `push: branches: [master]` as literally written in Task 1 — a real gap in the plan itself: Task 1's own Step 2 verification pushes to a feature branch, which would never have triggered a master-only workflow. Correct fix to a plan bug, not a deviation to flag as a problem.
- `ghcr.io/flathub-infra/flatpak-github-actions:gnome-50` as the lint container image (matching the manifest's runtime-version exactly) instead of the plan's guessed `bilelmoussaoui/...gnome-47` tag, and `flatpak run --command=flatpak-builder-lint org.flatpak.Builder <subcommand> <path>` as the real lint invocation shape instead of the plan's guessed `--lint-manifest`/`--lint-appstream` flags. Both are the plan's own flagged "mechanical-correction" unknowns, resolved correctly against reality.
- `flatpak-node-generator` needed a real workaround for two issues found in practice: (1) the primary dev environment is PEP 668 externally-managed, so generation ran in a scratch virtualenv rather than a system-wide `pip install`; (2) v0.1.1 treats already-installed local packages as sources and produces an incomplete offline cache unless run against a clean checkout (or with `node_modules` moved aside). Documented in `docs/packaging.md`.
- Two real Rust-side fixes landed (`770636c`), touching `crates/wield-cli/src/lib.rs` and `crates/wield-core/tests/command_runner.rs`, despite this plan's own Global Constraints implying the Rust side would stay untouched. Both are legitimate, narrowly-scoped bug fixes for real environment-specific flakiness surfaced only once this plan's new CI infrastructure existed to exercise it: (1) `wield-cli::run()` now validates args *before* the async portal probe, so invalid-input calls fail fast instead of always paying for a D-Bus round trip (also fixes a source of CI-only hangs when no portal service is reachable); (2) `command_runner.rs`'s `nonzero_exit_reports_stderr_tail_and_cleans_temp` test now execs `/bin/sh -c "<script>"` directly instead of writing a temp file and chmod+exec'ing it — the write-then-exec pattern raced on the GitHub Actions runner's filesystem. GEON reviewed both diffs directly; correct, minimal, necessary. Global Constraints' "Rust untouched" was a description of the expected outcome given already-working code, not a rule forbidding a real fix a new CI surface uncovered — same judgment call this project has made every time so far (P5a's exit-prevention bug, P2's borrow-checker bugs).

**GEON's continuation, after SUNIO's limit:**
- Diagnosed the Release workflow's actual failure via `gh run view --log-failed`: `flatpak-builder-lint` flagged `finish-args-unnecessary-appid-own-name` and `finish-args-portal-talk-name` as errors. Researched via Flathub's own linter source/docs (not guessed): portal interfaces (`org.freedesktop.portal.*`) are reachable from inside a Flatpak sandbox without an explicit `talk-name` grant — that exception "is never granted" per the linter's own exceptions list, meaning it's always considered a mistake to keep, not something to except — and an app's own D-Bus name is granted by default. **Dropped both from `finish-args`** (`8929493`) and amended the design spec's §9 to match, since the original spec had both listed as required. This is a real correction to a previously-locked spec block, not a plan bug — the spec was written before this project had ever actually run a real Flathub linter against a manifest.
- Fixed the Task 6 lint step to tolerate exactly two already-known, already-tracked findings (`metainfo-missing-screenshots`, `appstream-screenshots-not-mirrored-in-ostree` — the placeholder-icon/no-screenshots gap named since Task 5 was written) while still failing on anything else, rather than a blanket `|| true` that would silently swallow a real future regression (`3a5b160`). First draft used bash process substitution (`<(...)`); caught during local testing that the container's default shell for `run:` steps is `sh` (confirmed in an earlier failing run's log: `shell: sh -e {0}`), not `bash` — rewrote using temp files instead of process substitution so it runs correctly under strict POSIX `sh`. Verified the exact extracted shell function against three fixture cases (only-known-findings passes, clean passes, one unexpected finding fails) before pushing, rather than round-tripping through CI to find shell portability bugs.
- Iterated the disposable-tag verification loop three more times (`v0.1.0-p7-smoke2`, `-smoke3`) exactly as Task 6 Step 2 describes, deleting each after use; the Release workflow is now genuinely green (run `34625299334`), not just "probably fine." CI (`ci.yml`) confirmed green at the final HEAD too (run `34626386042`).
- Observed one CI-only test flake (`wield-core::command_runner::kills_on_timeout`, on the intermediate `8929493` run) — the exact class of pre-existing, load-sensitive flake this project's own history already anticipated needing "P7 CI tuning" for (see `docs/testing.md`'s P5a section). Passed clean on the very next push. Not chased into a fix here, consistent with this plan's own promise that the Rust side stays untouched apart from SUNIO's two already-justified fixes above — flagged here for visibility, not silently absorbed.
- Full local re-verification: `cargo fmt --all` (no diff), `cargo clippy --workspace --all-targets -D warnings` (clean), `cargo test --workspace` (all green, 0 failed), `npm run check` (20 files / 70 tests, clean), both new file-validity checks (metainfo XML, both vendored-sources JSON files) — all pass.

**Tasks 8 and 9, after explicit per-action confirmation (asked separately, both answered yes):**

- **Task 9:** added `test` as a required branch-protection status check on `master` via the GitHub API, preserving every existing setting (0 approvals, `enforce_admins=false`, no force-push/deletion, required conversation resolution). Verified via a follow-up API read.
- **Task 8:** Flathub's actual current submission process differs from this plan's guess in three real ways, discovered by reading `flathub/flathub`'s live `CONTRIBUTING.md` and `docs.flathub.org` directly rather than trusting the plan text: (1) PRs target the **`new-pr` branch**, not `master` — `flathub/flathub`'s `master` branch holds no app directories at all any more (checked directly: only `.github`, `CODEOWNERS`, `CONTRIBUTING.md`, `COPYING`, `README.md`), and `new-pr` is a genuinely empty branch (its tree is git's well-known empty-tree hash) that submitters branch from; (2) **only the manifest file goes in the submission**, committed directly at the branch root and named after the app-id — no per-app subdirectory; (3) the metainfo/desktop-file/icon must **not** be duplicated into the submission — Flathub's requirements page is explicit that those "must be integrated in the upstream project," which Wield's manifest already satisfies (it installs all three from this repo's own `packaging/flatpak/` at build time). Opened: https://github.com/flathub/flathub/pull/10176. Recorded in `docs/packaging.md`.
