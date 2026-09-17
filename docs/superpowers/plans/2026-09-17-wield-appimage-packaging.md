# AppImage Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Wield's Flatpak release pipeline with AppImage — turn on Tauri's native `appimage` bundler, and rewrite `release.yml` to build and attach it to a GitHub Release on tag push.

**Architecture:** A one-line `tauri.conf.json` change turns on a bundler this project has never actually exercised (`bundle.targets` is currently `[]`). `packaging/flatpak/`'s from-scratch manifest is removed entirely. `release.yml` is rewritten around `tauri-apps/tauri-action`, pinned to `ubuntu-22.04` (not `ubuntu-latest`) for glibc-compatibility reasons. This is infrastructure, not application code — there is no unit-testable surface; every task's real verification is either a config/local sanity check or, for the pipeline itself, an actual live GitHub Actions run.

**Tech Stack:** Tauri 2's bundler (`linuxdeploy`-based AppImage support), `tauri-apps/tauri-action` GitHub Action, GitHub Releases.

**Spec:** `docs/superpowers/specs/2026-09-17-wield-appimage-packaging-design.md`

## Global Constraints

- AppImage **replaces** Flatpak entirely — no Flatpak files, no Flatpak CI job survive this plan (spec §1).
- Converter-binary bundling (ffmpeg, ImageMagick, pandoc, qpdf/ghostscript, Tesseract, LibreOffice) is explicitly **out of scope** — do not add it, even if it looks easy while touching this area (spec §1, §5).
- The release job is pinned to `ubuntu-22.04`, never `ubuntu-latest` — building on a newer base bakes in a higher minimum glibc requirement that breaks the AppImage on older systems (spec §2).
- Releases are created as **drafts** (`releaseDraft: true`), not auto-published — this pipeline has never run for real before (spec §3.4).
- The real risk — [tauri-apps/tauri#14796](https://github.com/tauri-apps/tauri/issues/14796), `linuxdeploy` failing specifically in GitHub Actions CI, open and unresolved — must be validated against a live run, not assumed away (spec §2, §3.5). Task 4 is that validation; do not skip it or treat local success as sufficient.

---

### Task 1: Turn on Tauri's `appimage` bundler

**Files:**
- Modify: `apps/wield/src-tauri/tauri.conf.json`

**Interfaces:**
- Produces: a valid `bundle.targets: ["appimage"]` + `bundle.icon` config, consumed by Task 4's live build.

- [ ] **Step 1: Make the config change**

In `apps/wield/src-tauri/tauri.conf.json`, find:

```json
    "bundle": {
        "active": true,
        "targets": [],
        "category": "Utility",
        "shortDescription": "Portal-native Linux utility hub",
        "longDescription": "A command palette and tray for quick Linux actions and file conversions."
    }
```

Replace with:

```json
    "bundle": {
        "active": true,
        "targets": ["appimage"],
        "icon": ["icons/icon.png"],
        "category": "Utility",
        "shortDescription": "Portal-native Linux utility hub",
        "longDescription": "A command palette and tray for quick Linux actions and file conversions."
    }
```

(`icons/icon.png` already exists at `apps/wield/src-tauri/icons/icon.png` — it's the placeholder icon already tracked in `docs/backlog.md`, not something to replace as part of this plan.)

- [ ] **Step 2: Validate the config parses correctly**

Run, from the repo root:

```bash
python3 -c "import json; json.load(open('apps/wield/src-tauri/tauri.conf.json'))"
```

Expected: no output, exit code 0 (valid JSON). This is a cheap sanity check only — it does not confirm the bundler itself works; that's Task 4's job, deliberately, per this plan's Global Constraints.

- [ ] **Step 3: Confirm the Tauri CLI recognizes the new target**

Run:

```bash
npx --prefix apps/wield tauri build --help 2>&1 | grep -A3 "bundles"
```

Expected: help text listing `appimage` among the recognized bundle identifiers (confirms the installed `@tauri-apps/cli` version understands the target name — a real, if small, thing that could be wrong if the CLI version were old).

- [ ] **Step 4: Commit**

```bash
git add apps/wield/src-tauri/tauri.conf.json
git commit -m "tauri: turn on the appimage bundler target"
```

---

### Task 2: Remove `packaging/flatpak/`

**Files:**
- Delete: `packaging/flatpak/io.github.DhanushSantosh.Wield.yml`
- Delete: `packaging/flatpak/io.github.DhanushSantosh.Wield.desktop`
- Delete: `packaging/flatpak/io.github.DhanushSantosh.Wield.metainfo.xml`
- Delete: `packaging/flatpak/io.github.DhanushSantosh.Wield.png`
- Delete: `packaging/flatpak/cargo-sources.json`
- Delete: `packaging/flatpak/node-sources.json`

**Interfaces:** none — pure removal, nothing downstream depends on these files once Task 3 removes their only consumer (the `flatpak` job in `release.yml`).

- [ ] **Step 1: Remove the directory**

```bash
git rm -r packaging/flatpak
```

- [ ] **Step 2: Confirm nothing else references these files**

```bash
grep -rn "packaging/flatpak" --include="*.yml" --include="*.md" --include="*.json" . 2>/dev/null
```

Expected: no matches outside of `docs/packaging.md` and `docs/backlog.md` (both are documentation, updated in Task 3 and Task 5 respectively — not a reason to stop here, just confirms nothing *executable* still points at the removed directory).

- [ ] **Step 3: Commit**

```bash
git commit -m "packaging: remove the Flatpak manifest and its assets"
```

---

### Task 3: Rewrite `release.yml` and `docs/packaging.md`

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `docs/packaging.md`

**Interfaces:**
- Consumes: Task 1's `bundle.targets: ["appimage"]` config.
- Produces: the actual CI pipeline Task 4 validates live.

- [ ] **Step 1: Rewrite the workflow**

Replace the entire contents of `.github/workflows/release.yml` with:

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

jobs:
  appimage:
    runs-on: ubuntu-22.04
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
            librsvg2-dev \
            libgtk-layer-shell-dev

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Install Node
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm

      - run: npm ci

      - uses: tauri-apps/tauri-action@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          projectPath: apps/wield
          tagName: ${{ github.ref_name }}
          releaseName: "Wield ${{ github.ref_name }}"
          releaseBody: "See the AppImage below to download and run this version."
          releaseDraft: true
          prerelease: false
          args: --bundles appimage
```

This is spec §3.1 verbatim — pinned to `ubuntu-22.04` (not `ubuntu-latest`), the same apt dependency list `ci.yml` already proves works on this exact runner image, `projectPath: apps/wield` (confirmed live: `apps/wield/package.json` and `apps/wield/src-tauri/` both exist, matching `tauri-action`'s documented "root of the project... containing package.json, src-tauri/" contract), `releaseDraft: true` per the Global Constraints.

- [ ] **Step 2: Rewrite `docs/packaging.md`**

Replace the entire contents of `docs/packaging.md` with:

```markdown
# Packaging Wield

## AppImage (primary channel)

- `.github/workflows/release.yml` builds the AppImage on every `v*` tag push,
  via Tauri's own bundler (`tauri build --bundles appimage`, driven by
  `tauri-apps/tauri-action`) and attaches it to a **draft** GitHub Release.
  Nothing is published automatically — a human reviews and publishes the
  release once the artifact looks right.
- The release job is pinned to `ubuntu-22.04`, not `ubuntu-latest` —
  building on a newer base system bakes in a higher minimum glibc
  requirement, which can break the AppImage on older host systems. See
  Tauri's own AppImage distribution docs for this exact warning.
- `tauri.conf.json`'s `bundle.targets` includes `"appimage"`; `bundle.icon`
  points at `icons/icon.png` (the same placeholder icon tracked below, not
  replaced by this pipeline).
- `linuxdeploy` (Tauri's underlying AppImage tool) has an open, unresolved
  upstream report of failing specifically in GitHub Actions CI
  ([tauri-apps/tauri#14796](https://github.com/tauri-apps/tauri/issues/14796)).
  If a future release run hits this, the fallback is dropping `tauri-action`'s
  bundling step for a hand-rolled sequence in the same job: build the plain
  binary (`npm run build -w apps/wield -- --no-bundle`), invoke `linuxdeploy`
  and `appimagetool` directly as separate debuggable steps, then
  `gh release create`/`gh release upload` to attach the result.
- Converter binaries (ImageMagick, ffmpeg, pandoc, qpdf/Ghostscript,
  Tesseract, LibreOffice) are **not** bundled into the AppImage — every
  `Command`-capability tool reports `Unavailable` inside it today. This is
  real, separate, deliberately deferred follow-up work (see
  `docs/backlog.md`), not an oversight.
- Local AppImage builds need `linuxdeploy`/`appimagetool`, which Tauri's
  bundler downloads on first use — not pre-installed in the primary
  development sandbox, so the AppImage build is validated in CI (the same
  established pattern this file already had for Flatpak's own build
  requirements).

### Known gaps

- **Icon:** still the placeholder desk illustration in `assets/icon.png`
  (and now also referenced directly by the AppImage build via
  `apps/wield/src-tauri/icons/icon.png`, the same file). Replace before any
  real, publicly-announced release.
- **Bundled converter binaries:** see above — real follow-up work, not
  attempted by this pipeline.
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml docs/packaging.md
git commit -m "release: rewrite the pipeline around Tauri's AppImage bundler"
```

---

### Task 4: Validate against a real GitHub Actions run

**Files:** none — this task pushes a throwaway git tag and observes CI; no code changes.

**Interfaces:**
- Consumes: Tasks 1-3's complete pipeline.
- Produces: a confirmed-working (or confirmed-broken-with-a-clear-reason) `release.yml`, which Task 5 finalizes either way.

This is the task the whole plan exists to get right — per the Global
Constraints, the open `linuxdeploy`-in-CI issue is real and must be
checked against an actual run, not assumed away.

- [ ] **Step 1: Push the branch and a throwaway tag**

```bash
git push -u origin feat/appimage-packaging
git tag v0.0.1-test
git push origin v0.0.1-test
```

(The tag triggers `release.yml` regardless of which branch it's pushed
from — the workflow's trigger is `push: tags: - "v*"`, not scoped to any
particular branch.)

- [ ] **Step 2: Watch the actual run**

```bash
gh run list --workflow=release.yml --limit 1
gh run watch $(gh run list --workflow=release.yml --limit 1 --json databaseId --jq '.[0].databaseId')
```

Expected: the run completes. If it fails, read the failure directly — do
not guess. Two realistic failure shapes:

- **The apt/Rust/Node setup steps fail** — almost certainly unrelated to
  AppImage bundling specifically (this exact dependency list already works
  in `ci.yml`); investigate as a normal CI failure.
- **The `tauri-apps/tauri-action` step itself fails, specifically during
  bundling** (error text mentioning `linuxdeploy` or `appimagetool`) — this
  is the known, open risk. Apply the fallback described in `docs/packaging.md`
  (Task 3, Step 2): replace the single `tauri-action` step with explicit
  `tauri build --no-bundle` + manual `linuxdeploy`/`appimagetool` invocation
  + `gh release create`/`gh release upload` steps, commit that change to
  this same branch, delete the `v0.0.1-test` tag and release (Step 4 below),
  and re-push a fresh throwaway tag to validate the fallback path instead.
  Do not consider this task done until *some* working path is confirmed
  live — a documented-but-unverified fallback is not suffient given this
  plan's whole purpose is confirming the pipeline actually works.

- [ ] **Step 3: Confirm the draft release and its artifact**

```bash
gh release view v0.0.1-test --json isDraft,assets
```

Expected: `isDraft: true`, and `assets` containing one file whose name ends
in `.AppImage`. Download it and sanity-check it's real:

```bash
gh release download v0.0.1-test --pattern "*.AppImage" --dir /tmp/appimage-check
file /tmp/appimage-check/*.AppImage
```

Expected: `file` reports something like `ELF 64-bit LSB executable... (statically linked)` — not a truncated or zero-byte file. If a spare machine or VM is reasonably available, actually running it (`chmod +x`, then execute) and confirming the palette appears is a stronger check than `file` alone, but `file`'s confirmation plus a non-trivial size (tens of MB, not a few KB) is the minimum bar.

- [ ] **Step 4: Clean up the throwaway tag and release**

```bash
gh release delete v0.0.1-test --yes --cleanup-tag
git push --delete origin v0.0.1-test 2>/dev/null || true
git tag -d v0.0.1-test
rm -rf /tmp/appimage-check
```

(`--cleanup-tag` removes the remote tag too; the explicit `git push
--delete` afterward is a harmless no-op if it already went, and a real
cleanup step if `--cleanup-tag` didn't cover it for some reason — don't
skip verifying the tag is actually gone from `git ls-remote --tags origin`
before moving on.)

- [ ] **Step 5: No commit for this task**

Nothing changed in the tree — Task 4 is pure validation. If Step 2's
fallback path was needed, that fallback's code change was already
committed as part of re-running Step 1-3; this step remains a no-op in
the common case where `tauri-action` just worked.

---

### Task 5: Record what was verified

**Files:**
- Modify: `docs/backlog.md`
- Modify: `docs/testing.md`

**Interfaces:** none — documentation only.

- [ ] **Step 1: Update the stale `gtk-layer-shell` Flatpak entry in `docs/backlog.md`**

Find the existing entry (under `## Packaging`):

```markdown
- **`gtk-layer-shell` Flatpak build module — missing, will break the next
  release tag.** `layer_shell.rs` links the system `gtk-layer-shell` library;
  `ci.yml`'s ubuntu-latest runners install it directly, but
  `org.gnome.Platform`/`Sdk` don't bundle it. The next `release.yml` tag
  build will fail the same way CI initially did until a build module (source
  tarball + meson/ninja) is added to the manifest. More time-sensitive than
  the other packaging gaps here - the next tag push hits this, not just
  Flathub review.
```

Replace it with:

```markdown
- **~~`gtk-layer-shell` Flatpak build module — missing, will break the next
  release tag.~~ Resolved by dropping Flatpak entirely** (2026-09-17) —
  `release.yml` now builds an AppImage instead; `linuxdeploy`'s dependency
  tracing bundles `gtk-layer-shell` automatically since it's a real linked
  `.so` of `wield-app`. Full write-up:
  `docs/superpowers/specs/2026-09-17-wield-appimage-packaging-design.md`,
  live-verified per `docs/testing.md`'s AppImage section.
```

Also find the two related entries and update them to reflect the switch —
**Bundled converter binaries** (still real, now phrased against AppImage
instead of Flatpak) and **Flathub submission on hold** (still accurate as
written — Flathub was always Flatpak-specific and stays irrelevant now that
Flatpak itself is gone; leave that entry as-is, don't delete it, since it's
useful history for why Flatpak was abandoned rather than just fixed).

Update the "Bundled converter binaries" entry's first line from "are not
bundled in the Flatpak" to "are not bundled in the AppImage" and adjust the
rest of its prose (which tool reports `Unavailable` where) accordingly —
same underlying gap, different package format.

- [ ] **Step 2: Add a `docs/testing.md` section**

Read the file's existing structure first (tail of the file, matching the
established per-milestone section style) and add a new section following
the same heading/prose conventions, covering: the real GitHub Actions run
from Task 4 (link or run ID if convenient, the actual result), whether
`tauri-action` worked directly or the fallback was needed, and the
`file`-command / size confirmation of the downloaded `.AppImage`. Write
this once Task 4 is actually complete — it documents what really happened,
not a prediction.

- [ ] **Step 3: Commit**

```bash
git add docs/backlog.md docs/testing.md
git commit -m "docs: record the AppImage packaging switch and its live verification"
```

---

## After all tasks: PR

Per this project's standing workflow, GEON opens the PR from
`feat/appimage-packaging` against `master` after reviewing every commit
against this plan — do not merge without the user's explicit "merge it"
for this specific PR, even under broad delegation.
