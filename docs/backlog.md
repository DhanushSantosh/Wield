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

- **KDE/Sway/X11 not tested; GNOME partially tested, real findings, one
  question still open.** All Wayland layer-shell work (palette positioning,
  the full-screen click-dismiss backdrop) has only been live-verified on
  Hyprland. GNOME's fallback path (`layer_shell::is_available() == false`)
  got a real, if incomplete, live pass (2026-09-17) via a nested GNOME 50.4
  session (`gnome-shell --devkit --wayland --virtual-monitor`, GNOME's
  replacement for the old `--nested` flag - `mutter-devkit` was also
  installed, since `--devkit` alone runs the compositor headless with no
  viewer). Confirmed for real: `layer_shell::is_available()` correctly
  detects the absence of `wlr-layer-shell` and logs the expected fallback
  message. Confirmed **architecturally, not just by testing** - true
  regardless of how much further live-testing happens - that
  click-outside-to-dismiss cannot work on GNOME as currently built: the
  fallback is a small fixed-size centered window (`tauri.conf.json`), not
  the full-screen transparent surface layer-shell gives Hyprland, so there
  is no Wield-controlled area outside the card to ever catch an "outside"
  click; Escape and the tray are the only ways out there, same shape as the
  already-accepted cross-monitor dismiss gap below. **Left genuinely open**:
  whether the palette reliably shows and receives keyboard input at all
  under GNOME. One live attempt showed a real, visible, correctly-positioned
  window; two immediate retries in fresh sessions showed nothing at all, no
  error anywhere, D-Bus calls all reporting success - cross-checked with two
  independent screenshot tools to rule out a capture artifact. Read as the
  nested-`--devkit`-testing rig itself being fragile (a very new GNOME 50.4
  feature, `dbus-run-session` + a virtual monitor + the devkit viewer
  stacked together) rather than a finding about Wield on a real GNOME
  session - not treated as a confirmed bug either way. Revisit with a real
  (non-nested) GNOME session if GNOME support becomes an actual priority,
  not more nested-devkit testing.
- **Cross-monitor click-away-to-dismiss doesn't work.** Clicking a
  genuinely different, unfocused monitor while the palette is shown does not
  dismiss it - a real, confirmed Hyprland bug
  ([hyprwm/Hyprland#14136](https://github.com/hyprwm/Hyprland/discussions/14136)),
  not something client-side code can fully work around without one
  full-screen backdrop surface per connected output (real added complexity,
  scoped out as low-value versus the same-monitor case that matters most).
  Escape or the tray remain reliable there. Full writeup:
  `docs/testing.md`'s "Click-away-to-dismiss" section.
- **The window behind the palette can't be interacted with while Wield is
  open** — clicking it dismisses Wield first, then needs a second click on
  the now-visible target; there's no way to reach it in one action.
  Researched properly (2026-09-15), not just assumed: the standard Wayland
  mechanism for this (`wl_surface.set_input_region`, paired with
  `KeyboardMode::Exclusive` for the interactive area) is real and works
  correctly on sway and niri, confirmed via a live Hyprland discussion
  ([hyprwm/Hyprland#14136](https://github.com/hyprwm/Hyprland/discussions/14136)
  — same bug as the cross-monitor gap above) — but Hyprland specifically
  ignores input regions on exclusive layer surfaces, and the discussion is
  still open with zero replies. Separately, `gtk-layer-shell` (the library
  `layer_shell.rs` is built on) has no input-region API at all
  ([wmww/gtk-layer-shell#30](https://github.com/wmww/gtk-layer-shell/issues/30))
  — GDK's Wayland backend only syncs input regions for windows mapped
  through GTK's standard `xdg-shell` path, which layer-shell surfaces
  bypass; reaching it would mean dropping to raw GDK/Wayland calls beneath
  the library itself. A synthetic click-replay fallback (dismiss, then
  inject a matching click at the same screen position) was also spiked and
  rejected: `ydotool`'s absolute positioning on this dev machine isn't a
  direct pixel mapping (empirically verified `input × 2 + 1`, clamping
  near screen edges — a property of this exact dual-monitor setup, not
  portable), and would also need per-monitor offset lookups and an
  external daemon dependency (`ydotoold`) just to fake one click. Even
  Rofi — the most established launcher in this category — has open user
  reports of the identical symptom with equivalent settings enabled
  ([davatorium/rofi#885](https://github.com/davatorium/rofi/issues/885)),
  suggesting this is a genuinely hard spot for this whole app category on
  current Wayland compositors, not a Wield-specific gap. Owner decision:
  accept the two-click reality for now; revisit only if Hyprland or
  `gtk-layer-shell` fix their end.
- **`keep.awake` doesn't prevent actual system suspend on Hyprland, only
  idle/screensaver lock.** Live-verified (2026-09-17): `keep.awake` requests
  `InhibitFlags::Idle | InhibitFlags::Suspend` together, the portable, correct
  request per the real `Inhibit` portal spec. `xdg-desktop-portal-hyprland`'s
  own backend only honors `Idle` - every `Inhibit()` call logs `"A backend
  call failed: Inhibiting other than idle not supported"` on the main
  `xdg-desktop-portal` process, though the overall call still succeeds and
  returns a valid `Request`, so this fails silently from the caller's side.
  Not a Wield gap - an `xdg-desktop-portal-hyprland` backend limitation.
  Full write-up: `docs/testing.md`'s "M3b: `keep.awake`" section.
- **A hard crash or `kill -9` of Wield while `keep.awake` is active leaves
  the inhibitor held, with no way to release it short of logging out or
  restarting the session.** Verified against the real XDG portal spec during
  M3b's design (2026-09-17): `org.freedesktop.portal.Inhibit`'s `Inhibit()`
  call has no auto-release on connection loss - release is
  `Request.Close()` on the specific handle it returned, and nothing calls
  that on an unclean exit. The tray's Quit item does close it on a normal
  quit (`docs/testing.md`'s M3b section verifies this), which covers the
  common case; a crash or force-kill bypasses that path entirely. Inherent
  to the portal's own design, not something client code can close.

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
