# Backlog

Deferred work and known gaps that aren't being actively worked, tracked here
so they don't get lost. Each item names where its full detail lives, if any.
Not a roadmap — see `docs/superpowers/specs/2026-09-10-wield-design.md` §6
for the milestone plan (M1–M5).

## Packaging

- **Icon.** `assets/icon.png` is still the placeholder desk illustration
  inherited from the pre-Wield DeskCrafter project. Replace before requesting
  final Flathub approval. Full detail: `docs/packaging.md`'s "Known gaps".
- **AppStream screenshots.** None published yet; add a `<screenshots>` block
  after release-quality UI captures exist.
- **Bundled converter binaries.** ImageMagick, pandoc, qpdf, Ghostscript,
  Tesseract+`eng`, `libreoffice-fresh`, and the `ffmpeg-full` runtime extension
  are not bundled in
  the Flatpak — `color.pick` works there, everything `Command`-backed
  (`image.convert`, and M2's `video.convert`/`audio.extract`/
  `document.convert`/`pdf.tools` as they land) reports `Unavailable` inside
  the Flatpak specifically, while working fully on native installs via
  `$PATH`. Each binary needs real from-scratch Flatpak build-module research
  (checked `flathub/shared-modules` during P7 — nothing ready-made exists for
  any of these). Re-scoped, not forgotten: explicitly deferred again when
  starting M2 (2026-09-14, owner decision) — Flathub submission itself is on
  hold, so bundling for a Flatpak that isn't being submitted yet is
  low-urgency.
- **`document.convert` can't read legacy binary Office formats
  (`.doc`/`.ppt`/`.xls`).** `pandoc` has no reader for them at all
  (confirmed via `pandoc --list-input-formats` - only the modern XML-based
  `docx`/`pptx`/`xlsx` are supported). Reading them would need the
  engine-selection logic to also branch on the *input*'s format, not just
  the requested output format - real added complexity, deliberately
  deferred rather than built into M2c's first version. `libreoffice`
  itself can read these formats fine; only the routing logic is missing.
- **`gtk-layer-shell` Flatpak build module — missing, will break the next
  release tag.** `layer_shell.rs` links the system `gtk-layer-shell` library;
  `ci.yml`'s ubuntu-latest runners install it directly, but
  `org.gnome.Platform`/`Sdk` don't bundle it. The next `release.yml` tag
  build will fail the same way CI initially did until a build module (source
  tarball + meson/ninja) is added to the manifest. More time-sensitive than
  the other packaging gaps here - the next tag push hits this, not just
  Flathub review.
- **Secondary channels** (AUR, `.deb`, `.rpm`) deferred to a later packaging
  plan; only Flatpak is being pursued for `1.0`.
- **Flathub submission on hold.** PR
  [flathub/flathub#10176](https://github.com/flathub/flathub/pull/10176) was
  auto-closed by Flathub's own bot — their checklist requires confirming no
  AI tool opened the PR, which isn't true here. Owner decision: hold the
  whole submission until MVP is further along; fork + branch left in place,
  untouched, for whenever the owner wants to submit it personally.

## Platform coverage

- **KDE/Sway/GNOME(no-layer-shell fallback)/X11 not tested.** All Wayland
  layer-shell work (palette positioning, the full-screen click-dismiss
  backdrop) has only been live-verified on Hyprland. The GNOME/X11 fallback
  path (`layer_shell::is_available() == false`) is code-reviewed and unit-
  gated but not live-tested on an actual GNOME or X11 session.
- **Cross-monitor click-away-to-dismiss doesn't work.** Clicking a
  genuinely different, unfocused monitor while the palette is shown does not
  dismiss it - a real, confirmed Hyprland bug
  ([hyprwm/Hyprland#14136](https://github.com/hyprwm/Hyprland/discussions/14136)),
  not something client-side code can fully work around without one
  full-screen backdrop surface per connected output (real added complexity,
  scoped out as low-value versus the same-monitor case that matters most).
  Escape or the tray remain reliable there. Full writeup:
  `docs/testing.md`'s "Click-away-to-dismiss" section.

## Converter tooling (M2)

- **`ReportResult` (multi-file batch outcome) has no actions.** `FileResult`
  offers "Open folder"/"Run again"; the batch summary card
  (`ReportResult.tsx`) renders plain text only, so a user can't click
  through to any of the converted files. Every M2 tool is a batch tool via
  `Executor::run_batch`, so this is worth closing before M2b-d repeat the
  pattern. Flagged by the M2a final review.
- **`video.convert`'s `quality` (CRF) help text assumes libx264.** The
  descriptor offers `mp4`/`webm`/`mkv`; libvpx-vp9 (used for `webm`) has a
  0-63 CRF scale (not 0-51) and wants `-b:v 0` alongside `-crf` for true
  constant-quality mode. Not a bug (the current command still succeeds),
  but the copy is misleading for 2 of the 3 offered formats. Flagged by
  the M2a final review.
- **`quality`'s `[0, 51]` range makes `0` (lossless, huge output) reachable
  by accident.** `FormField.tsx`'s numeric coercion treats a cleared field
  as `0`, and `0` is a valid, meaningful CRF value here (unlike
  `image.convert`'s `[1, 100]` range, which is immune). Affects any future
  optional `Int` arg whose valid range includes 0 - a `FormField.tsx`-level
  fix (distinguish "empty" from "0") would close it for every tool at
  once, not just this one. Flagged by the M2a final review.
- **`video.convert`'s 1800s timeout may be tight for `webm` (libvpx-vp9) at
  default speed settings**, which is meaningfully slower than libx264. Not
  hit in testing; noted as a risk for a long 1080p-to-webm conversion.
- **`compute_output_path`'s `OutputDir::SameAsInput` hardcodes the literal
  arg name `"input"`** (`template.rs`), while `run_batch` is generic over
  whichever arg holds the `ArgValue::Paths`. A future multi-file arg not
  named `input` would silently fail per-file with `MissingInputArg` rather
  than erroring at descriptor-build time. Pre-existing, made newly visible
  by `run_batch`'s genericity. Flagged by the M2a final review.
- **`FfmpegDuration` (the ffmpeg progress parser) lives directly in
  `command.rs`.** Fine for one parser; worth splitting into its own
  `progress/` submodule once M2c/M2d (`document.convert`, `pdf.tools`) add
  their own `ProgressParser` impls.
- **`pdf.split`'s discovered-file order is alphabetical, not numeric.**
  At 10+ pages, `qpdf`'s own `%d`-numbered output (`page-1.pdf`,
  `page-10.pdf`, `page-2.pdf`, ...) sorts lexicographically wrong before
  `Executor::run_split` renumbers them - each split file's *content* is
  still correct (exactly one real page each), but which page ends up
  labeled `-2` vs `-10` in the final filename could be off for larger
  documents. Not fixed for M2d: a numeric-aware sort is a small,
  legitimate follow-up, not core-mechanism work.
- **`pdf.compress` has no batch support**, unlike every other M2
  converter tool. It could reasonably take `multiple: true` (independent
  per-file compression, exactly `run_batch`'s existing shape) - left out
  of M2d's scope deliberately, to keep this milestone's real
  scope (two new wield-core mechanisms) from also absorbing a third,
  unrelated addition.

## Misc

- **LICENSE still says "Copyright (c) 2025".** Inherited from the original
  scaffold, never updated. Flag if it ever matters (e.g. before a real
  public release announcement).
- **`agent-comms agent delete --id VIREO` still pending.** VIREO (the
  retired architect session) is already REVOKED and functionally inert;
  full delete just needs the owner's key and is optional.
