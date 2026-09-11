# Packaging Wield

## Flatpak (primary channel)

- Manifest: `packaging/flatpak/io.github.DhanushSantosh.Wield.yml`
- AppStream metadata:
  `packaging/flatpak/io.github.DhanushSantosh.Wield.metainfo.xml`
- Vendored sources must be regenerated after `Cargo.lock` or
  `package-lock.json` changes:

  ```bash
  python3 flatpak-cargo-generator.py Cargo.lock \
    -o packaging/flatpak/cargo-sources.json
  python3 -m flatpak_node_generator --no-requests-cache \
    -o packaging/flatpak/node-sources.json npm package-lock.json
  ```

  `flatpak-cargo-generator.py` comes from
  `flatpak/flatpak-builder-tools/cargo`. Install `flatpak-node-generator` in an
  isolated Python environment. The primary Arch development environment is
  externally managed under PEP 668, so the P7 generation run used a temporary
  virtual environment rather than modifying the system Python installation.
  Run the Node generator from a clean checkout, or temporarily move the root
  `node_modules` directory aside. Version 0.1.1 treats installed packages as
  local sources and otherwise emits an incomplete offline cache.

- `.github/workflows/release.yml` builds and lints the Flatpak on every `v*`
  tag and uploads `wield.flatpak` as an artifact.
- The manifest asks Tauri for its release binary with `--no-bundle`, then
  installs the app-ID-named desktop file and metadata directly. Tauri's Debian
  bundler probes for a host AppIndicator package that is intentionally absent
  from the GNOME SDK even though Wield loads tray support dynamically.
- Local full builds require Flatpak and Flatpak Builder. Those tools are not
  installed in the primary development sandbox, so the Flatpak build is
  validated in CI.
- `finish-args` dropped two entries the original design spec listed:
  `--talk-name=org.freedesktop.portal.Desktop` and
  `--own-name=io.github.DhanushSantosh.Wield`. `flatpak-builder-lint`
  (Flathub's own linter) flags both — portal interfaces are reachable from
  inside the sandbox without an explicit `talk-name` grant, and an app's own
  D-Bus name is granted by default. The spec has been amended to match
  (`docs/superpowers/specs/2026-09-10-wield-design.md` §9).
- The Flatpak lint step tolerates exactly two known findings
  (`metainfo-missing-screenshots`, `appstream-screenshots-not-mirrored-in-ostree`
  — both covered under "Known gaps" below) and fails on anything else, rather
  than suppressing the whole lint step.

### Known gaps

- **Icon:** the package still uses the placeholder desk illustration in
  `assets/icon.png`; the Flatpak directory contains a mechanically scaled and
  padded 512×512 copy for export. Replace both with a Wield-specific icon
  before requesting final Flathub approval. No pipeline change is required.
- **AppStream screenshots:** no screenshots are published yet. Add a
  `<screenshots>` block after release-quality UI captures exist.
- **Bundled converter binaries:** ImageMagick, pandoc, qpdf and Ghostscript,
  Tesseract with English data, and the `ffmpeg-full` runtime extension are not
  bundled. Inside the current Flatpak, `color.pick` is available and
  `image.convert` reports `Unavailable` because `magick` is absent. A focused
  P7-tools follow-up will package these dependencies.
- **Secondary channels:** AUR, `.deb`, and `.rpm` distribution are deferred to
  a later packaging plan.

## Flathub submission status

The submission manifest and metadata are prepared. Opening a public pull
request against `flathub/flathub` remains pending the fresh confirmation gate
in Task 8 of the P7 implementation plan.
