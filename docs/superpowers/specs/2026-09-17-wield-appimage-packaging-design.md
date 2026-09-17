# Wield — AppImage Packaging Design

## 1. Why this exists

`release.yml` currently builds a Flatpak on every `v*` tag push, via a
from-scratch manifest (`packaging/flatpak/io.github.DhanushSantosh.Wield.yml`)
that has zero build modules besides the app itself — no `gtk-layer-shell`,
which Wield links directly and `org.gnome.Platform`/`Sdk` don't bundle. The
next tag push would very likely fail that build outright (tracked in
`docs/backlog.md` since M2). Flathub submission is also separately on hold
(owner decision — the submission PR was auto-closed by Flathub's own bot;
held until the MVP is further along), so this blocker sits in front of a
release channel that isn't even active yet.

AppImage replaces it: Tauri 2's bundler has native `appimage` support,
unlike the Flatpak manifest's from-scratch situation — its `linuxdeploy`
dependency tracing should bundle `gtk-layer-shell` (a real linked `.so` of
`wield-app`) automatically, the way it already does for every other linked
library. No Flathub-style review process stands in front of it either;
GitHub Releases is a real, immediate distribution channel.

**Owner decisions locking this scope (2026-09-17):**
- AppImage **replaces** Flatpak entirely — `packaging/flatpak/` and its
  `release.yml` job are removed, not kept alongside.
- Converter binaries (ffmpeg, ImageMagick, pandoc, qpdf/ghostscript,
  Tesseract, LibreOffice) are **not** bundled as part of this effort —
  deferred as separate follow-up work, tracked in `docs/backlog.md`, matching
  this project's repeated pattern (M2d, M3a, M3b all kept new-mechanism work
  separate from adjacent scope).
- The built AppImage is attached to a **public GitHub Release** on tag push
  (created as a **draft** — see §3.4) — the obvious channel given no
  Flathub-style index is active.

## 2. What's already there, verified against the actual tooling and docs

- **`tauri.conf.json`'s `bundle.targets` is currently `[]`.** Tauri's own
  bundler has never actually been exercised in this project — the Flatpak
  build's `npm run build -w apps/wield -- --no-bundle` step deliberately
  skips it, manually installing the raw binary via Flatpak's own
  `install -Dm755`. Turning on `appimage` is this project's first real use
  of Tauri's bundler, not a switch between two already-working ones.
- **`tauri-apps/tauri-action`** (confirmed via its own repo) is the
  official, standard GitHub Action for building Tauri apps and optionally
  creating/uploading to a GitHub Release in one step — the documented
  pattern most Tauri projects use, not something to hand-roll.
- **A real, unresolved risk, found via fresh research, not assumed away**:
  [tauri-apps/tauri#14796](https://github.com/tauri-apps/tauri/issues/14796)
  is an open (as of this design, "needs triage") report of
  `linuxdeploy`-based AppImage bundling failing specifically in GitHub
  Actions CI, across multiple Ubuntu versions and Docker, with no
  documented fix found by the reporter. This project's existing `ci.yml`
  already proves the *underlying binary* builds fine on GitHub-hosted
  Ubuntu runners with `libgtk-layer-shell-dev` installed — the risk is
  specifically the AppImage-*bundling* step, not the app build itself. This
  is why §4 makes live CI validation an explicit early step, not an
  assumption.
- **Tauri's own docs carry a real, separate gotcha**: build on the *oldest*
  base system you want to support, not the newest — a newer build host
  bakes in a higher minimum glibc requirement, and an AppImage built on a
  too-new Ubuntu can fail on older systems with `version 'GLIBC_2.33' not
  found`. `ci.yml`'s jobs use `ubuntu-latest` (rolling, currently a newer
  Ubuntu) — fine for testing, but the release-building job pins to
  `ubuntu-22.04` explicitly instead (§3.1), specifically because of this.
- **`bundle.linux.appimage` in `tauri.conf.json` only exposes two options**
  (confirmed against the current Tauri config reference):
  `bundleMediaFramework` (pulls in ~15-35MB of gstreamer for audio/video
  playback — Wield doesn't play media, stays unset/false) and `files` (extra
  files to include — unused for this design, since converter-binary
  bundling is explicitly deferred, §1). There's no separate `.desktop`-file
  or icon path to hand-author for AppImage specifically — Tauri generates
  both from the top-level `bundle` config (`productName`,
  `shortDescription`, `category`, and `bundle.icon`).
- **`bundle.icon` is not currently set at all** in `tauri.conf.json` —
  verified directly. `apps/wield/src-tauri/icons/icon.png` already exists
  (420KB, a single PNG, no multi-size variants) but nothing in the bundle
  config references it yet, because nothing has ever bundled before. This
  is the same placeholder icon already tracked in `docs/backlog.md`
  ("the placeholder desk illustration inherited from the pre-Wield
  DeskCrafter project") — using it here doesn't add a new gap, it's already
  a known one, tracked separately.

## 3. Design

### 3.1 `.github/workflows/release.yml` — rewritten

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

Pinned to `ubuntu-22.04` (§2's glibc reasoning), not `ubuntu-latest`. The
apt dependency list is copied verbatim from `ci.yml`'s already-proven set —
same exact packages, same exact build succeeds on this runner image today.
`projectPath: apps/wield` points `tauri-action` at the actual Tauri project
(the workspace root isn't it — `apps/wield/src-tauri` is, and `tauri-action`
resolves `projectPath` to the directory *containing* `src-tauri`, matching
how `npm run build -w apps/wield` already scopes to this workspace member).
`tagName: ${{ github.ref_name }}` uses the literal pushed tag directly
(e.g. `v0.1.0`) rather than tauri-action's own `__VERSION__`-substitution
convention — simpler, and avoids the tag needing to exactly match
`tauri.conf.json`'s `version` field for the substitution to make sense.

### 3.2 `apps/wield/src-tauri/tauri.conf.json` — bundle config

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

Two changes from the current file: `targets` goes from `[]` to
`["appimage"]` (the actual switch that turns Tauri's bundler on for this
target), and a new `icon` array is added, pointing at the icon file that
already exists but was never referenced by anything (§2). `category`,
`shortDescription`, `longDescription` are unchanged — already correct,
previously inert since nothing bundled.

### 3.3 `packaging/flatpak/` — removed

All five files removed: the manifest
(`io.github.DhanushSantosh.Wield.yml`), `.desktop` file, `metainfo.xml`,
icon, and the two dependency-lockfile-equivalents
(`cargo-sources.json`, `node-sources.json`). Their *content* isn't wasted
work, even though the files go away — the `.desktop` entry's name/exec/
category and the AppStream metadata's description text are exactly what
Tauri's own bundler now generates from `tauri.conf.json`'s `bundle` fields
instead (§3.2) — the same information, moved to the one config Tauri's
bundler actually reads, rather than duplicated in a second, now-dead
manifest format.

### 3.4 Release as a draft, not published immediately

`releaseDraft: true` (§3.1) — the GitHub Release is created but not made
public until manually published. This is the first time this project's
release pipeline will have actually run end-to-end; treating the very
first real tag push as "build it, let a human look at the actual release
page and the actual AppImage before anyone else can see it" is the safer
default for a pipeline this new, not a permanent policy choice. Revisit
once the pipeline has proven itself across a couple of real tags.

### 3.5 Validating the real risk — not assumed away

Before this is considered done, the actual GitHub Actions build of the
AppImage needs to succeed for real, not just look right on paper — directly
because of the open, unresolved `linuxdeploy`-in-CI issue (§2). The
implementation plan's first task pushes a throwaway tag (e.g. `v0.0.1-test`,
deleted afterward) to trigger `release.yml` for real and confirms: the
workflow completes, a `.AppImage` file is actually attached to the
resulting draft release, and — as far as can be checked without a second
machine — the file looks like a real AppImage (correct size range, `file`
reports it as an ELF executable with the AppImage magic bytes present).

**If it hits the same `linuxdeploy` failure that issue describes**, the
fallback is dropping `args: --bundles appimage`/`tauri-action`'s bundling
step for a hand-rolled sequence in the same job: `cargo tauri build
--no-bundle` (or the existing `npm run build -w apps/wield -- --no-bundle`)
to produce the plain binary, then invoke `linuxdeploy` and `appimagetool`
directly as separate, debuggable shell steps, then `gh release create`/
`gh release upload` to attach the result — more pipeline to maintain, but
each step becomes independently inspectable if something fails, which
`tauri-action`'s single opaque step is not. This fallback is not written
out in full here — it's real work only worth doing if §3.5's validation
actually fails, not speculative work to build alongside the primary path.

## 4. Testing

Nothing here is unit-testable in the conventional sense — this is a CI/
build-pipeline change, not application code. Verification is entirely
live, matching §3.5: a real tag push, a real GitHub Actions run, a real
draft release with a real `.AppImage` attached, inspected directly (file
type, rough size sanity-check, and — if reasonably possible — actually
running it, e.g. via a VM or spare machine, to confirm the packaged binary
launches and the palette appears, not just that the file exists).

## 5. Out of scope (tracked separately)

- Converter binary bundling (§1) — real, separate follow-up work.
- The placeholder icon (§2) — already tracked in `docs/backlog.md`, this
  design doesn't newly introduce it, just makes it visible in one more
  place (the AppImage's own icon) than it was before.
- AppImage auto-updates (`AppImageUpdate` + zsync) — not built; not needed
  until there's more than one real release to update *from*.
- ARM builds — Tauri's own docs note `linuxdeploy` can't cross-compile ARM
  AppImages (native ARM runners or emulation only); not pursued now, no
  ARM users known to be waiting on this.
