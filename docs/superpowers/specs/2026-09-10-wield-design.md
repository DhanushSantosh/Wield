# Wield — Design Spec

**Date:** 2026-09-10
**Status:** Approved for planning

> **Repository note.** Wield began as a pivot of the DeskCrafter project. On
> 2026-09-10 the owner deliberately reset the project: the source tree was
> re-homed to `github.com/DhanushSantosh/Wield`, git history was discarded,
> and the old DeskCrafter repo was removed. There is **no history to
> preserve, no archive branch, and no archived launcher code**. This spec
> is written for a fresh repository whose working tree currently still
> carries DeskCrafter naming (transformed in plan P1).

---

## 1. Why Wield exists

DeskCrafter was a GUI suite for an individual to create and repair their own Linux
application launchers. That framing had a structural weakness: creating or fixing a
`.desktop` file is a roughly once-a-year action for most people, and mature tools already
cover it (`menulibre`, `AppImageLauncher`, a text editor). Low frequency means low
retention, which is why the project stalled without a compelling use case.

What was worth keeping is the shape of the engine — a Rust core that models a desired
desktop state, validates it, and reconciles/repairs. The GUI shell and the launcher
domain itself were the least valuable parts.

**Wield** reuses the Tauri + React + Rust stack to build a different product: a polished,
always-available Linux utility hub — a command palette plus tray that fronts a suite of
quick actions (screen capture tools) and file/data converters.

### The architectural bet: portal-native

The Linux desktop is fragmented across desktop environments (GNOME, KDE, Xfce, tiling
WMs) and display servers (Wayland per-compositor protocols, X11). Building DE-specific
integrations is a treadmill. **XDG Desktop Portals** (`xdg-desktop-portal`, accessed over
D-Bus) are the freedesktop-designed common layer: an app calls a documented portal
interface, and `xdg-desktop-portal` routes it to a DE-maintained backend. Wield codes
once against the portal; the per-DE work is someone else's responsibility.

**Scope rule (the YAGNI knife):** if a capability has no portal and no safe standardized
`ext-*` Wayland protocol, it is out of scope. This deliberately excludes:

- Arbitrary window control / tiling (FancyZones-style) — no portal, excluded from
  Wayland standardization on security grounds. GNOME/KDE tile natively anyway.
- Synthetic input (text expansion, macros, paste-as-keystrokes) — requires going below
  Wayland to `/dev/uinput` via a privileged daemon. Deferred to an optional, separately
  packaged add-on (M5); the core suite never needs elevated privileges.

The converter half of the product does not touch portals at all — it is process
orchestration over bundled CLIs (`ffmpeg`, `ImageMagick`, `pandoc`, `libreoffice`,
`qpdf`), portable by construction.

---

## 2. Decisions locked

| Decision | Choice |
| --- | --- |
| Name | **Wield** — `wield` binary, crates `wield-core` / `wield-portal` / `wield-tools` / `wield-cli`, app-id `io.github.DhanushSantosh.Wield` (`wield.dev` + `wield.app` are third-party-registered; GitHub-namespace id chosen 2026-09-10) |
| Repository | `github.com/DhanushSantosh/Wield`, fresh history, branch `master` |
| Foundation | Portal-native (`ashpd` + `zbus`); CLI orchestration for converters |
| Privileges | Core suite = zero. Optional input daemon = separate package, milestone M5 |
| Shell | Global-hotkey command palette (primary) + tray / StatusNotifierItem (anchor + fallback) |
| Stack | Tauri + React + Rust core; single pre-warmed hidden palette window |
| Tool model | Descriptor-driven executor (Approach B) |
| Scope rule | No portal / safe `ext-*` protocol → out of scope |
| Old launcher code | **Discarded.** `launcher.doctor` (M4) is a from-scratch rebuild, not a port — see §6 |
| v1 | Thin vertical slice first: shell + one Portal tool + one Command tool + packaging pipeline |
| Distribution | Flatpak / Flathub primary; AUR + `.deb`/`.rpm` secondary; AppImage skipped for v1 |

### Name-collision check (performed 2026-09-10)

`Wield` is clear on every namespace that matters for Linux distribution: no Debian/apt
package, no AUR package, no Flathub app, no trademarked company, no same-category
product, clean SEO. Bare `wield` on crates.io (v0.1.0, April 2021) and npm (v0.0.1,
2016) are abandoned stubs — hence the `wield-*` crate prefix; the `$PATH` binary `wield`
has no conflict.

Rejected: **Knack** (existing `knack` apt package — Microsoft's Azure CLI framework;
plus knack.com no-code company), **Haft** (`haft.tools` is an active AI-coding-tools
CLI — overlapping audience), **Fettle** / **Portico** / **Adroit** (active software
products / companies in-category).

---

## 3. System architecture

### Crates

| Crate | Responsibility |
| --- | --- |
| `wield-core` | Descriptor model; the executor; capability runners (`Portal` / `Command` / `Native`); tool registry; dependency resolution; `ToolOutcome` |
| `wield-portal` | Thin `ashpd` wrapper for the portals Wield uses; startup capability probe (interfaces + versions available); degradation reporting |
| `wield-tools` | Built-in tool descriptors (data) + the `Native` tool implementations (`screen.ocr` pipeline, `launcher.doctor`) |
| `wield-cli` | CLI surface generated from the same registry (`wield ocr`, `wield image.convert …`); keeps the registry surface-agnostic |

### App

`apps/wield` (transformed from `apps/desktop`) — Tauri shell.

- **Rust side** links `wield-core` + `wield-tools`; exposes Tauri commands: `list_tools`,
  `run_tool`, `cancel`, `capabilities`.
- **React side** renders the palette, per-tool argument forms **generated from each
  descriptor's args schema**, result cards, Preferences, and first-run onboarding.

### Site

`apps/site` — copy and screenshots rewritten for Wield. The existing cinematic
design-system work is retained.

### Data flow (one invocation)

```
global hotkey ──▶ GlobalShortcuts portal activation ──▶ shell shows pre-warmed palette window
palette ──▶ list_tools (registry, fuzzy search over title + keywords)
pick tool ──▶ if args: render form from schema
              if portal-picker / no args: run immediately
run_tool(id, args) ──▶ wield-core executor:
     Portal(adapter)  → ashpd call (e.g. Screenshot.PickColor)
     Command(spec)    → resolve binary, render argv from template, spawn,
                        stream progress, collect output file / error
     Native(fn)       → call Rust fn
result ──▶ ToolOutcome ──▶ result card
              (copyable value / open folder / report / error with fix or hint)
```

### Binary resolution

`Command` tools resolve their binary through the executor: a **bundled path inside the
Flatpak**, or `$PATH` lookup on non-Flatpak installs. `ffmpeg` / `ImageMagick` / `pandoc`
/ `qpdf` / `tesseract` are bundled into the Flatpak build; no `flatpak-spawn --host`.

---

## 4. Descriptor & executor model (Approach B)

### Tool descriptor

Every tool — built-in or user-supplied — has the same shape:

| Field | Meaning |
| --- | --- |
| `id` | Stable namespaced string, e.g. `image.convert` |
| `title` | Display name |
| `keywords` | Extra terms for palette fuzzy search |
| `category` | `Capture` \| `Convert` \| `Desktop` |
| `args` | Ordered list of `ArgSpec` |
| `requires` | `Portal { iface, min_ver }` \| `Binary("ffmpeg")` \| `None` |
| `output` | `Value(Color \| Text \| …)` \| `File { … }` \| `Report` |
| `capability` | `Command { … }` \| `Portal { adapter }` \| `Native { fn }` |

### ArgSpec

`name`, `label`, `help`, `type`, `default`, `required`, optional
`when { arg, in: [...] }` for conditional visibility.

`type` ∈ `file` (with `filters`, `multiple`) \| `dir` \| `string` \| `int`
(`range`, `step`) \| `float` \| `bool` \| `enum` (`options`) \| `text`.

The React arg form and the CLI flags are both generated from this list. No per-tool UI
code.

### The three capabilities

**`Command` — pure data.** Where the converter suite lives.

```
capability = Command {
  binary   = "ffmpeg"
  args     = ["-i", "{input}", "-vf", "scale={width}:-1", "{output}"]
  output   = File { name = "{input_stem}.{format}", dir = SameAsInput }
  progress = FfmpegPipe          # optional stderr -> percent parser
  timeout  = 5m
  success  = ExitZero
}
```

The executor is fully generic for these: render argv from template + validated args,
spawn, stream progress, collect the output file or a structured error.

Template placeholders include `{argname}` for each arg plus derived values
`{input_stem}`, `{input_dir}`, `{format}`.

**`Portal` — descriptor + named adapter.** There are ~5–6 portal tools ever; each adapter
is a small reused function in `wield-portal`, referenced by key:

```
capability = Portal { adapter = "screenshot.pick_color" }
```

Adapters for v1: `screenshot.pick_color`, `screenshot.region`, `inhibit.toggle`.

**`Native` — descriptor + Rust fn.** Rare. The escape hatch for real logic
(`screen.ocr` = screenshot portal → tesseract → text cleanup; `launcher.doctor`).

All three share the args schema, search metadata, dependency requirements, output typing,
and every generated surface (palette / tray / CLI / any future D-Bus API).

### Executor pipeline

1. **Validate** args against the schema (types, ranges, `required`, `when` visibility).
2. **Check `requires`** — portal probe result / binary resolved? If not →
   `Unavailable { reason, fix }`; tool shown greyed in the palette with the fix text.
3. **Render** the invocation.
4. **Execute** with a cancellation token + timeout.
5. **Progress** reported over a channel → Tauri event → React progress bar.
6. **Outcome** — a single enum:
   `Value { kind, data }` \| `File { path }` \| `Report { … }` \|
   `Unavailable { reason, fix }` \| `Failed { stage, detail, hint? }` \| `Cancelled`.
7. **Post-actions** — copy value / open folder / run again with retained values.

### Registry & descriptor storage

- **Built-ins:** type-safe Rust builders in `wield-tools` — no runtime parse risk.
- **User-supplied:** TOML in `$XDG_CONFIG_HOME/wield/tools/`, **`Command`-only** (no
  arbitrary native or portal code from config), parsed and validated at load; invalid
  descriptors are skipped with a logged warning.
- Registry exposes: list, get, fuzzy-search (title + keywords), filter-by-availability.
- **Registry snapshot test** guards accidental descriptor changes.

---

## 5. The shell

### Palette window

A single **frameless, centered, always-on-top Tauri window, created hidden at startup and
never destroyed.** Show/hide on activation keeps the webview warm so it opens instantly.

- Empty query → recent / most-used tools. Typing → fuzzy search over title + keywords.
- Keyboard-driven: arrows + Enter; Esc hides; blur hides (configurable).
- Pick a tool → if it has args, the palette expands into the generated form; a
  portal-picker or no-args tool runs immediately.
- Result shown inline as a card: copyable value / "open folder" / report. "Run again"
  returns to the form with values retained.

### Tray (StatusNotifierItem)

Always-visible anchor. Left-click opens the palette; the menu lists tools by category plus
Preferences and Quit; stateful tools (`keep.awake`) show active state. On GNOME this needs
the AppIndicator extension — if SNI registration finds no host, Wield says so once and
points to the hotkey.

### Global shortcut

- **Primary:** the `GlobalShortcuts` portal, registered on first run (default
  `Super+W`). GNOME/KDE show their own binding-confirmation UI; the session token is
  stored. Tauri's own `globalShortcut` API is X11-only and is **not** used.
- **Fallback** (portal absent — older systems, some wlroots compositors): Preferences
  shows a copyable `wield palette` command and instructions to bind it in the DE's
  keyboard settings; the tray becomes the primary entry point.

### Lifecycle

- Launches headless: tray + hidden palette + registered shortcut; no main window.
- Autostart is opt-in during onboarding via the `Background` portal.
- **Single instance owns the D-Bus name `io.github.DhanushSantosh.Wield`** with `ShowPalette` /
  `RunTool` methods. Portal activation, the tray, and `wield palette` from a second launch
  all route through it.
- CLI with no instance running (`wield image.convert file.png`) runs the tool one-shot,
  no shell.

### Preferences

Separate normal window: hotkey setup, launch-at-login, palette behavior (blur-to-hide,
clear-query), default output directory, per-tool enable/disable, and a **System Status
page** — which portals and binaries were detected, and why any given tool is unavailable.

### Capability probe (`wield-portal`, at startup)

Introspects `org.freedesktop.portal.Desktop` for interfaces + versions (`GlobalShortcuts`,
`Screenshot` incl. `PickColor` version, `ScreenCast`, `Inhibit`, `Background`,
`FileChooser`) and resolves `Command` binaries. Feeds the palette (grey out + reason), the
System Status page, and a first-run summary if something notable is missing. Wield never
blocks startup on a missing capability.

---

## 6. v1 tool set & milestones

### Tools

| id | Category | Capability | Backing | Notes |
| --- | --- | --- | --- | --- |
| `color.pick` | Capture | Portal | `Screenshot.PickColor` | Result copyable as hex / rgb / hsl |
| `screen.ocr` | Capture | Native | Screenshot portal → tesseract → cleanup | Text to clipboard; `eng` bundled, more downloadable |
| `keep.awake` | Capture | Portal | `Inhibit` | Stateful toggle; tray shows active state |
| `image.convert` | Convert | Command | ImageMagick | format + resize + quality, `when`-gated |
| `video.convert` | Convert | Command | ffmpeg | format + resolution + quality |
| `audio.extract` | Convert | Command | ffmpeg | extract / convert audio track |
| `document.convert` | Convert | Command | pandoc (+ libreoffice headless fallback) | md / docx / html / rst / … |
| `pdf.tools` | Convert | Command | qpdf / ghostscript | compress / merge / split |
| `launcher.doctor` | Desktop | Native | **from-scratch rebuild** | scan + repair broken `.desktop` |

### Milestones

- **M1 — Architecture slice.** Shell (palette + tray + hotkey + `io.github.DhanushSantosh.Wield` D-Bus
  + capability probe) · `color.pick` (proves the Portal path) · `image.convert` (proves
  the Command path + generated arg form + progress + result card) · Flatpak packaging
  pipeline. End-to-end through the whole stack. Decomposed into sequential plans P1–P7.
- **M2 — Converter suite.** `video.convert`, `audio.extract`, `document.convert`,
  `pdf.tools`. Output-directory rules, stderr→progress parsers, multi-file batch.
- **M3 — Capture.** `screen.ocr` (Native), `keep.awake` (Inhibit). Screen recorder
  deferred — ScreenCast + PipeWire is its own effort.
- **M4 — Desktop.** Build `launcher.doctor` **from scratch** — FreeDesktop `.desktop`
  entry parsing, validation against the Desktop Entry Specification, `Exec`/`TryExec`
  resolution, icon-theme lookup, and a repair pass, exposed as a `Native` tool. This is
  a genuine build, not a lift; budget it as roughly a converter-suite-sized milestone on
  its own, with a substantial test surface (spec-compliance fixtures, repair heuristics).
  Also bring desktop-integration actions (`autostart` → Background portal;
  `default_apps` / `flatpak` overrides → `Command` via `xdg-mime`, `xdg-settings`,
  `flatpak override`) in as descriptors.
- **M5 — Optional input daemon.** Separate package: text expansion, paste-as-keystrokes,
  macros, over `/dev/uinput` with polkit + a systemd unit. Behind a clean IPC boundary;
  the core suite works fully without it.

`1.0` is tagged when M1–M3 are shipped and stable.

### Portal / dependency notes

- `PickColor` needs Screenshot portal v2+; older → tool greyed with reason.
- OCR region selection is compositor-provided via the interactive Screenshot portal;
  `tesseract` + `eng` traineddata bundled in the Flatpak.
- `Inhibit`, `FileChooser`, `Background` are broadly available — low risk.
- All `Command` binaries bundled in the Flatpak → converters have no host dependencies;
  non-Flatpak installs fall back to `$PATH` detection.

---

## 7. Error handling

### Outcome taxonomy

| Outcome | Cause | Presentation |
| --- | --- | --- |
| `Cancelled` | User cancels a portal dialog or hits Stop | **Silent** — return to palette, no card |
| `Unavailable { reason, fix }` | `requires` unmet (portal missing / too old, binary absent) | Tool greyed in palette; specific fix text, e.g. "Screenshot portal v2 needed; GNOME 42 has v1 — update to 43+" |
| `Failed { stage, detail, hint? }` | Portal error, nonzero exit, missing output, timeout, FS error, native panic | Result card: one-line summary + `hint` + expandable raw detail + retry / copy-details |

`stage` ∈ `Validation` \| `Portal` \| `Command` \| `Output` \| `Native`.

### Rules

- **User-cancel is always silent.** Only genuine failures get a card.
- **Never surface a raw stack trace or D-Bus error string as the primary message** —
  that is behind an expand.
- **Arg validation runs before execution**; field errors map back onto the generated
  form, not a card.
- **Common failure patterns get hints:** ffmpeg "Invalid data found" → "input may be
  corrupt or mislabeled"; ImageMagick "no decode delegate" → "missing codec for this
  format"; sandbox path denial → "open the file through the picker so Wield is granted
  access".
- **Native tools are wrapped at the Tauri command boundary** (`catch_unwind` →
  `Failed`) — a native tool never crashes the shell.
- **Runtime re-check:** a tool available at probe time but failing `requires` at run time
  (portal service died) re-probes and updates the palette.

### Atomic output

`Command` tools write to a temp file in the destination directory, `fsync`, then
atomic-rename into place **only on success**. A failed or cancelled conversion never
leaves a half-written file.

### Cancellation

Every run has a token surfaced as a Stop button. Portal → drop the `ashpd` request /
close the session. Command → SIGTERM to the process group, 3s grace, SIGKILL; temp output
discarded.

### Logging

`tracing` → `$XDG_STATE_HOME/wield/logs` with rotation. Per invocation: tool id, rendered
argv, outcome, duration. "Copy details" bundles that slice. **No telemetry, no network.**

---

## 8. Testing strategy

### `wield-core` (unit)

- **Argv rendering** — template + args → expected argv. Paths with spaces, omitted
  optional args, enum substitution, `{input_stem}` / `{format}` expansion. Highest-value
  surface — where Command bugs live.
- **Arg schema** — type coercion, range enforcement, `required`, `when` visibility
  resolution.
- **Descriptor validation** — bad `when` arg refs, duplicate ids, missing fields
  rejected.
- **Outcome mapping** — exit code + stderr fixture → expected `Failed { stage, hint }`.
- **Binary resolution** — Flatpak bundled path vs `$PATH` (mocked).
- **Registry snapshot test** — serialize the built-in registry to a snapshot.

### `wield-portal` (unit)

- Capability probe: fake interface/version map → correct availability set.
- Adapter response → `ToolOutcome` mapping (ashpd call mocked).

### Executor (integration, real subprocesses)

- Stub scripts (echo argv / fixed exit code / fake ffmpeg progress on stderr) verify
  progress parsing, timeout kill, cancellation, atomic rename, temp cleanup.
- A few **real** conversions against bundled `ffmpeg` / `ImageMagick` / `pandoc` with
  tiny fixtures (1-frame mp4, 2×2 png, 3-line md), CI-gated where deps are present —
  proves the descriptors are actually correct.

### Portal path

- **Mock portal D-Bus service** — a test fixture owns `org.freedesktop.portal.Desktop`
  on a private bus and returns canned `PickColor` / `Screenshot` / `Inhibit` responses.
  Covers wiring without a compositor.
- **Manual matrix** (documented checklist): GNOME current + one old, KDE Plasma,
  sway / Hyprland — run each portal tool, confirm degradation messaging on old GNOME.

### Frontend (Vitest + Testing Library, Tauri layer mocked)

- Arg-form generator: descriptor → form; `when` show/hide; validation display.
- Palette: fuzzy ranking, keyboard nav, greyed tools + reason tooltip.
- Result card per outcome variant.

### E2E smoke (CI: xvfb + dbus session + mock portal)

App boots headless → tray registers or reports SNI absence → owns `io.github.DhanushSantosh.Wield` →
`RunTool` executes `image.convert` on a fixture → output file appears. One happy path
through the whole stack.

### Out of scope

Real compositor portal behavior in CI (manual matrix instead); third-party CLI internals
(test our invocation, not `ffmpeg` itself).

---

## 9. Packaging & distribution

### Primary: Flatpak / Flathub

- **App-id** `io.github.DhanushSantosh.Wield` (locked 2026-09-10 — `wield.dev` and
  `wield.app` are registered to third parties; the GitHub-namespace id needs no domain
  verification).
- **Runtime** `org.gnome.Platform` (pinned) — provides webkitgtk for the Tauri webview.
- **Bundled tool binaries:** `ffmpeg` via the `org.freedesktop.Platform.ffmpeg-full`
  runtime extension (not built from source); `ImageMagick`, `pandoc`, `qpdf` +
  `ghostscript`, `tesseract` + `eng` traineddata as Flatpak modules.
- **`finish-args` (deliberately minimal):**
  ```
  --socket=wayland  --socket=fallback-x11  --share=ipc  --device=dri
  --talk-name=org.freedesktop.portal.Desktop
  --talk-name=org.kde.StatusNotifierWatcher
  --own-name=io.github.DhanushSantosh.Wield
  ```
  **No `--filesystem=host`.** Files enter exclusively through the FileChooser portal
  (per-file grants via the document portal). Consequence: inside the sandbox the palette's
  converters always go through the portal picker — no arbitrary paths. CLI one-shot mode
  with bare paths is a non-Flatpak feature.

### Secondary

| Channel | Notes |
| --- | --- |
| **AUR** (`wield`, `wield-git`) | PKGBUILD in `packaging/aur/`; `Command` binaries resolve from `$PATH`; deps `webkit2gtk ffmpeg imagemagick pandoc qpdf tesseract tesseract-data-eng` |
| **`.deb` / `.rpm`** | Tauri bundler; CLI deps as `Depends` / `Requires`. Lower priority |
| **AppImage** | Skipped for v1 — bundling ffmpeg / tesseract is heavy and fragile |

### CI / release

GitHub Actions on tag: run full test suite + E2E smoke → build Flatpak (flatpak-builder
in a container), `.deb` / `.rpm` (tauri-action). Canonical manifest in
`packaging/flatpak/`; Flathub gets a manifest PR. Semver — `0.x` across milestones,
`1.0` when M1–M3 are stable.

### Updates

Flatpak → the store. Native → distro / AUR. No in-app auto-updater. An opt-in "check for
updates" setting is post-`1.0` and the only permitted network call.

### First-run onboarding

Detects install type (Flatpak vs native) + portal / binary availability, then walks
through: register the global shortcut (portal), opt into launch-at-login (Background
portal), show System Status. Skippable.

---

## 10. Starting point & initial restructure

The working tree at project root still carries DeskCrafter naming. Plan **P1** transforms
it in place; there is no history migration and no archived code.

| Current | Target |
| --- | --- |
| `crates/core` (launcher domain: `desktop_entry`, `icons`, `inspectors`, `launcher_library`, `platform`, `safety`, `types`, `tools/*`) | **Deleted.** Replaced by `crates/wield-core` (descriptor model + executor, built in P2). The launcher domain is not carried over; `launcher.doctor` is rebuilt from scratch at M4. |
| — | New crates: `wield-core`, `wield-portal`, `wield-tools`, `wield-cli` |
| `apps/desktop` | `apps/wield` — Tauri `productName` "Wield" / identifier `io.github.DhanushSantosh.Wield`. Backend reduced to a placeholder in P1; hidden palette + tray land in P5. |
| `apps/site` | Copy / screenshots rewritten for Wield (separate plan; keep the cinematic design-system work) |
| `package.json`, npm scripts, root `Cargo.toml`, `README.md` | Renamed to Wield in P1 |
| `docs/*` (DeskCrafter product docs), `ONBOARDING.md`, stale `.claude/worktrees/*` copy | Removed or rewritten in P1; full `docs/` rewrite to the descriptor/executor model is a separate plan |

### User state

No prior userbase — no config migration. Wield uses `~/.config/wield`,
`~/.local/state/wield` fresh.

---

## 11. Repo artifacts to produce (across M1)

- `packaging/flatpak/io.github.DhanushSantosh.Wield.yml` + helper files
- `packaging/aur/PKGBUILD` + `PKGBUILD.git`
- Tauri bundler config for `.deb` / `.rpm` in `apps/wield/src-tauri/tauri.conf.json`
- Rewritten `docs/` (architecture, backend contracts, packaging, product scope, testing);
  `docs/tool-specs.md` → descriptor catalog
- `.github/workflows/` — CI (none exists today)
- Manual portal test matrix checklist in `docs/testing.md`

---

## 12. Open questions / risks

1. **Domain.** ~~Is `wield.dev` obtainable?~~ **RESOLVED 2026-09-10:** `wield.dev` and
   `wield.app` are both registered to third parties (since 2019). App-id locked to
   `io.github.DhanushSantosh.Wield` (GitHub namespace, no domain verification needed).
2. **First commit shape.** **RESOLVED 2026-09-10:** clean scaffold, zero DeskCrafter
   content in git history. P1 runs with no intermediate commits (see the GEON amendment
   at the top of the P1 plan); GEON makes the single first commit after review.
3. **`GlobalShortcuts` portal reach.** Needs GNOME 45+ / Plasma 5.27+. The fallback
   (user binds `wield palette` in DE settings) must be genuinely usable, not an
   afterthought — validate it on an older GNOME during M1.
4. **Flatpak `finish-args` review.** The minimal permission set must actually satisfy
   every v1 tool. If a tool needs more, that is a scope-review trigger, not a permission
   bump.
5. **Sandbox file access UX.** Every converter invocation opening a portal file picker
   may feel heavy for power users. Acceptable for v1; revisit whether a "recent files"
   convenience within granted scope helps.
6. **Tauri webview palette latency.** The pre-warmed hidden window should make it feel
   instant; measure on M1 and fall back to a lighter palette rendering only if it does
   not.
7. **libreoffice as a `document.convert` fallback** is a large dependency to bundle.
   Decide during M2 whether office-format conversion is worth the Flatpak size or is
   deferred.
8. **`screen.ocr` language packs.** `eng` bundled; the download-more flow needs a
   trusted source and a sandbox-friendly storage location (`$XDG_DATA_HOME/wield/tessdata`).
9. **M4 effort.** `launcher.doctor` as a from-scratch rebuild (§6) is materially larger
   than the original port-based plan. Re-scope M4 explicitly when M3 completes.
