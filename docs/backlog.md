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
  Tesseract+`eng`, and the `ffmpeg-full` runtime extension are not bundled in
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

## Misc

- **LICENSE still says "Copyright (c) 2025".** Inherited from the original
  scaffold, never updated. Flag if it ever matters (e.g. before a real
  public release announcement).
- **`agent-comms agent delete --id VIREO` still pending.** VIREO (the
  retired architect session) is already REVOKED and functionally inert;
  full delete just needs the owner's key and is optional.
