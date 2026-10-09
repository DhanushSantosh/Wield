# Packaging Wield

The primary distribution artifact is a Linux x86_64 AppImage, built on Ubuntu 22.04 for an older glibc floor. The [release playbook](releasing.md) describes beta/stable gates, drafts, and publication. Tauri's bundler builds the AppImage; `gh` creates the draft because the `tauri-action` release API path failed in this repository's earlier live tests (see `docs/testing.md`).

## Converter availability

The AppImage currently bundles Wield, **not** ImageMagick, FFmpeg, Pandoc, LibreOffice, qpdf, Ghostscript, or Tesseract. Wield discovers external executables via `PATH`, so a capability can work when a compatible helper is installed on the host; it is not guaranteed on a clean machine. Beta release notes must list these host prerequisites. Do not advertise a fully self-contained converter suite until clean-machine tests verify every advertised operation.

The staged stable-release solution is to bundle a tested core set of helper executables and required data as Tauri sidecars/resources, resolve bundled binaries deliberately, and retain documented host fallback where appropriate. Start with a single helper such as Pandoc, then expand after license and cross-distribution tests. Tesseract also needs traineddata; ImageMagick needs delegate/configuration coverage; FFmpeg requires codec/build-license review; qpdf needs its linked libraries; Ghostscript redistribution needs AGPL/commercial-license review; LibreOffice needs a separate relocatable-runtime spike. A binary's mere presence does not establish conversion support.

For each bundled converter, record its upstream version, source URL and digest, license and notice obligations, architecture, dynamic libraries, resource files, and real conversion smoke tests from a clean AppImage environment. Do not download converters silently at application startup. See [the primary-source research note](research/2026-10-09-release-and-converter-packaging.md).

## Known gaps

- The icon remains a placeholder. Replace it with a square production icon before a public announcement.
- Converter binaries and data remain host dependencies pending the staged packaging work above.
