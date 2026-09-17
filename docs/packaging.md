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
  replaced by this pipeline). **The icon must be genuinely square** —
  Tauri's bundler panics outright (`couldn't find a square icon to use as
  AppImage icon`) if it isn't. Found live: the placeholder was 815×813, two
  pixels off, which was enough to fail the build entirely.
- The job needs `permissions: contents: write` — GitHub's own default
  `GITHUB_TOKEN` permission for this repo is read-only, and `tauri-action`
  needs to create/update a release. Found live (`Resource not accessible by
  integration`), not assumed; without this the bundle itself succeeds but
  the release-creation step fails.
- `linuxdeploy` (Tauri's underlying AppImage tool) has an open upstream
  report of failing specifically in GitHub Actions CI
  ([tauri-apps/tauri#14796](https://github.com/tauri-apps/tauri/issues/14796)).
  **Live-verified this does not affect this pipeline** — the actual bundling
  step (`linuxdeploy` + the AppImage plugin, downloaded fresh by Tauri's
  bundler) completed successfully on `ubuntu-22.04` in this repo's CI; the
  two real failures hit instead were the icon and permissions issues above,
  both now fixed. If a future run somehow does hit the same failure that
  issue describes, the fallback is dropping `tauri-action`'s bundling step
  for a hand-rolled sequence in the same job: build the plain binary
  (`npm run build -w apps/wield -- --no-bundle`), invoke `linuxdeploy` and
  `appimagetool` directly as separate debuggable steps, then `gh release
  create`/`gh release upload` to attach the result.
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
