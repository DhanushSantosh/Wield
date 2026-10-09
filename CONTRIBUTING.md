# Contributing to Wield

Wield is a Linux/Tauri application. Open an issue for large changes so the behavior and platform scope can be agreed before implementation. Participation is covered by the [code of conduct](CODE_OF_CONDUCT.md). For security problems, follow the [security policy](SECURITY.md) and report privately instead of opening a public issue.

## Development

Use Node 24 or newer, npm 11 or newer, a Rust toolchain with `rustfmt` and `clippy`, and the [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux). Build on a Linux host with WebKitGTK 4.1. Run:

```bash
npm ci
npm audit --audit-level=high
cargo audit            # cargo install cargo-audit --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked -- --test-threads=1
npm run check
```

For native/portal/UI changes, include a real desktop smoke test in the PR. CI also runs the ignored launch test inside Xvfb and a fresh D-Bus session. Keep changes focused, add regression tests, update docs when behavior changes, and disclose any new external executable or license obligation.

## Pull requests

Ordinary contributions target `master` and use the default PR template. For a beta release PR targeting `master`, choose [the beta template](.github/PULL_REQUEST_TEMPLATE/beta-release.md); for a stable promotion PR targeting the planned `release` branch, choose [the stable template](.github/PULL_REQUEST_TEMPLATE/stable-release.md). In GitHub's PR creation URL, append `?template=beta-release.md` or `?template=stable-release.md` (use `&template=` if the URL already has a query string). GitHub does not automatically select a template by target branch. The stable template is preparatory: do not use that path until `release` protection, CI targeting, and tag ancestry checks are configured. The protected `master` branch requires the `test` check to pass and one approving review that isn't from the person who pushed last; new commits dismiss earlier approvals, and open review conversations must be resolved. A GitHub ruleset permits only `DhanushSantosh` to update `master`.

- **Contributor PRs:** the maintainer reviews, approves, and merges them. Nobody else can merge.
- **Maintainer PRs:** there is currently no second maintainer, so GitHub cannot supply the required approval. The maintainer merges these with the administrator bypass, and only after the `test` check is green and the PR's verification section is filled in. If a second maintainer joins, maintainer PRs go through normal review instead.

Never force-push `master`, and never use the bypass to merge a PR whose checks are failing.

## Releases

See [the release playbook](docs/releasing.md). Beta tags (`vX.Y.Z-beta.N`) point to `master`; stable tags (`vX.Y.Z`) point to the planned `release` branch. Both channels create drafts for maintainer review. A tag must match every version manifest and point to a commit already merged into its channel branch. Run the checked-in release gate locally before pushing the tag. CI repeats the gate and checks the uploaded asset, so local success is a prerequisite, not a substitute for CI. Do not use the stable branch path until its protection and CI are live.
