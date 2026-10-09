# Stable release promotion: vX.Y.Z

<!-- Use this template for a short-lived promotion PR targeting release. The release branch protection, CI targeting, and stable-tag ancestry checks must be configured before this path is used. Replace examples and leave unmet checks unchecked. -->

## Identity and promotion scope

- Target branch: `release` (not `master`)
- Proposed tag: `vX.Y.Z`
- Source beta tag/commit and last stable tag:
- Selected changes and PRs from `master`:
- Excluded or deferred changes (explain why):
- Stable-only fixes and the PR/plan to carry each back to `master`:
- User-visible changes, breaking changes, migration/rollback guidance, and release notes:

## Before squash-merge

- [ ] This is a short-lived promotion PR; the diff against `release` contains only intended stable changes, with no accidental inclusion of newer `master` work.
- [ ] The stable version matches `apps/wield/package.json`, `apps/wield/src-tauri/tauri.conf.json`, `package-lock.json`, all six crate manifests, and `Cargo.lock` is updated.
- [ ] `node scripts/release-check.mjs manifest vX.Y.Z` passes with the proposed tag.
- [ ] `npm ci`, `npm audit --audit-level=high`, and `cargo audit` pass; disclose any advisory warnings.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked -- --test-threads=1`, and `npm run check` pass.
- [ ] Real desktop and clean-machine AppImage tests cover changed UI, portal, native, and each advertised converter operation; record distribution/session and results below.
- [ ] Release notes state supported platforms, known limitations, converter prerequisites, and any bundled binary's license/notice obligations.
- [ ] The `release` branch's required CI checks and review policy are satisfied; no failing check is bypassed.
- [ ] Confirm the squash-merge result will be a new `release` commit; beta testing of the source commit does **not** validate this new commit.

Verification evidence (source SHAs, host, commands, results, screenshots/log links):

## After squash-merge — before tag push

<!-- Record these results in a PR comment if the PR body can no longer be edited. The stable tag must point to the squash commit on release. -->

- [ ] Record the squash-merged `release` SHA and confirm the working tree is clean and `HEAD` is on `origin/release`.
- [ ] Run `bash scripts/release-local.sh vX.Y.Z` on that exact SHA; record the full result and AppImage checksum.
- [ ] Re-run the local gate if the source, version, lockfile, or release scripts changed after the recorded pass.
- [ ] Only after local success, create and push an annotated tag on that SHA. Do not move or reuse a `v*` tag.
- [ ] GitHub release CI passes and creates a draft with one AppImage and `SHA256SUMS`; verify the downloaded assets.
- [ ] Review notes, architecture, licenses, and clean-machine launch/conversions before publishing; publish as **stable**, with **prerelease disabled** and **latest enabled**. Verify `/releases/latest` afterwards.
- [ ] Stable-only fixes are merged or tracked for inclusion in `master`.

Tag SHA, local-gate result, draft URL, publication decision, and backport links:

## Risks and recovery

<!-- Describe compatibility, rollback, and security risks. A bad immutable tag or published release requires a new version, not a rewritten tag or asset. -->
