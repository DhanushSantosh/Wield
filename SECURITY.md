# Security policy

## Supported versions

Wield is pre-1.0 and maintained by one person. Only the most recent published release (stable or beta) receives security fixes. Older releases are not patched; upgrade to the latest release.

## Reporting a vulnerability

Report vulnerabilities privately through [GitHub's private vulnerability reporting](https://github.com/DhanushSantosh/Wield/security/advisories/new). Do not open a public issue, pull request, or discussion for a suspected vulnerability.

Include the Wield version or commit, your distribution and desktop session, steps to reproduce, and the impact you observed. Remove personal paths, tokens, and other private data from logs before attaching them.

## What to expect

This is a best-effort, single-maintainer project, so there is no guaranteed response time. You will get an acknowledgement in the advisory thread once the report is read. A fix ships as a new release with a security note; tags and published assets are never rewritten. Credit is given in the advisory unless you ask otherwise.

## Scope

In scope: Wield's own code, its release artifacts (the AppImage and `SHA256SUMS`), and how Wield invokes external tools — for example, building a command line from user input, handling file paths, or using temporary files.

Out of scope: vulnerabilities inside the external converters Wield runs from your `PATH` (ImageMagick, FFmpeg, Pandoc, qpdf, Ghostscript, Tesseract, LibreOffice), and in the desktop portals or compositor. Report those to the upstream project. If you are unsure whether an issue is Wield's, report it here anyway.
