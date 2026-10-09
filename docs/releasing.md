# Wield release playbook

The supported release channels are beta (`vX.Y.Z-beta.N`) and stable (`vX.Y.Z`). Both create GitHub **drafts**. Publishing is a separate maintainer review action. `latest` must remain on a stable release; a beta is always a prerelease and never latest.

The channel split uses [a beta PR template](../.github/PULL_REQUEST_TEMPLATE/beta-release.md) for `master` and [a stable promotion PR template](../.github/PULL_REQUEST_TEMPLATE/stable-release.md) for `release`. The stable branch path becomes active only after `release` exists with protection, required CI, and its workflow files; do not tag a stable promotion before verifying those controls. GitHub selects these templates through the PR URL's `template` query parameter, not from the target branch automatically.

## 1. Prepare a release candidate PR

Update the version in the Tauri config, app `package.json`, npm lockfile, all six crate manifests, and Cargo.lock. Beta manifests include the `-beta.N` suffix. Add release notes covering changes, known limitations, and converter prerequisites. Run the ordinary development checks and a real desktop smoke test for affected surfaces. Open a beta PR to `master` with the beta template. For stable, select tested changes through a short-lived promotion PR targeting `release`, use the stable template, and squash-merge after review and CI. A squash creates a new commit, so beta validation does not validate the stable commit. Carry any stable-only fixes back to `master`. Contributor-authored PRs need the maintainer's approval; maintainer-authored ones use the documented administrator bypass because there is no second maintainer (see `CONTRIBUTING.md`). Never bypass a failing check.

Do not tag an unmerged branch. Once merged, record the exact `master` commit for beta or `release` squash commit for stable. Do not reuse or move a released tag: make a new version for a correction.

## 2. Prove the candidate locally

On a Linux host with Docker, run from a clean checkout of the chosen commit. The wrapper builds a pinned Ubuntu 22.04 container with Node 24 and Rust, then runs the shared gate inside it:

```bash
bash scripts/release-local.sh v0.1.0
```

Replace the example tag with the candidate tag. Before building, the wrapper refuses to run if the working tree has any uncommitted or untracked files, if `HEAD` is not reachable from `origin/master` for beta or `origin/release` for stable, or if the tag already exists locally pointing somewhere other than `HEAD`. The gate validates tag syntax and all manifest versions, installs locked npm dependencies, audits npm (high severity and above) and Rust (`cargo audit`, known vulnerabilities) dependencies, checks Rust format/lint/tests and app checks, builds an AppImage, validates its ELF/AppImage header and filename, and writes/verifies `SHA256SUMS`. Afterwards the wrapper fails if the gate changed any tracked file, such as a lockfile; CI runs the same check. CI invokes the **same script** on the tag, then downloads and checks the draft assets. Record the local commit SHA, command result, host distribution, and smoke-test evidence in the release PR. If source, version, lockfile, or release script changes, rerun the gate.

The gate uses `NO_STRIP=1` because the `linuxdeploy` helper's bundled `strip` cannot parse newer ELF `.relr.dyn` sections on some hosts; it uses AppImage extraction mode so build machines do not need FUSE. The container also avoids a Fedora GDK Pixbuf layout mismatch observed during local packaging. CI independently builds on Ubuntu 22.04, and artifact size may be larger.

This local prerequisite is an operator procedure: a tag-push workflow cannot attest that an untrusted local command ran. It still independently repeats all gates on GitHub. Never claim local success if the command or a needed runtime test was skipped.

## 3. Tag only after local success

Confirm the local HEAD equals the merged channel-branch SHA and CI is green. The release job rejects lightweight tags, beta commits not reachable from `master`, and stable commits not reachable from `release`.

```bash
git tag -a v0.1.0 -m "Wield v0.1.0"
git push origin v0.1.0
```

Use the approved release tag instead of the example. Only the maintainer should push release tags. Do not push another tag while the first release draft is unresolved. If CI fails, fix via a new commit and new tag; do not rewrite a published tag. If a draft exists but an asset is wrong, investigate before deleting/replacing it, and retain the failed run evidence. Stable corrections go to `release` and are then carried back to `master`.

## 4. Review and publish the draft

Check that the draft's tag, channel, commit, notes, architecture, AppImage, and `SHA256SUMS` agree. Verify the downloaded asset and exercise a clean-machine launch on the oldest supported distribution and representative desktop sessions. Test at least one real operation in each advertised capability; document unavailable converters. Review licenses and notices for any bundled helper binary. Check no secrets or unexpected files appear in the artifact.

For beta, keep prerelease enabled and latest disabled. For stable, keep prerelease disabled; mark it latest only when publishing. GitHub may treat a draft's latest setting differently from a published release, so verify `/releases/latest` after stable publication. Prefer publishing in the GitHub UI after this review. If using `gh`, explicitly inspect the release afterward:

```bash
gh release edit v0.1.0 --draft=false --latest
gh release view v0.1.0 --json isDraft,isPrerelease,tagName,url
```

For beta, use `gh release edit v0.1.0-beta.1 --draft=false --prerelease --latest=false` instead. Never run either example without changing the tag and completing the checklist.

## 5. Recovery and edge cases

- Invalid tags or manifest mismatches fail before building; a draft is not created.
- The release job is serialized per tag and refuses to overwrite an existing release. A retry after draft creation needs manual inspection, not silent asset replacement.
- A GitHub runner or dependency-download failure is not a local pass; rerun after investigating. Keep Ubuntu 22.04 for the AppImage glibc floor until compatibility testing supports a change.
- A broken published release should be superseded with a new version and advisory notes, not a moved tag or silently replaced asset.
- `v*` tags cannot be updated or deleted by anyone, including the maintainer (the "Release tags are immutable" ruleset has no bypass). A mistyped or accidental `v*` tag is permanent unless an admin temporarily disables that ruleset, deletes the tag, and re-enables it, recording why in the release PR. If the tag already created a draft, delete the draft first. Run `node scripts/release-check.mjs manifest <tag>` before every `git push origin <tag>` so a typo never reaches the remote.
- Never push throwaway `v*` tags to test pipeline changes here; they would be permanent. Test changes to `release.yml` in a personal fork instead, where tags can be deleted freely.
- Every workflow job has a `timeout-minutes` limit (release build 60, draft 15, CI jobs 30), so a hung download or build fails visibly instead of running for GitHub's six-hour default.
- If the `draft` job fails after `build` succeeded, the built artifact is kept for 30 days. Re-run only the failed job from the Actions UI rather than pushing a new tag, after checking why it failed and that no partial release exists.
- A new `cargo audit` or `npm audit` advisory can start failing the gate on a commit that passed before. Fix it with a dependency update PR; don't skip the audit. `cargo audit` currently reports 9 unmaintained/unsound warnings in the GTK/Tauri stack (including `gtk-layer-shell`); warnings don't fail the gate, only vulnerabilities do.
- Dependabot opens weekly PRs for pinned GitHub Actions SHAs, npm, Cargo, and the release image's Ubuntu digest (digest only — never a move off 22.04). The Node version and checksum in `.github/release.Dockerfile` are pinned by hand and must be bumped manually.
- Release artifacts are not yet signed or attested; `SHA256SUMS` proves integrity only against the same release page. Adding GitHub artifact attestations is deferred follow-up work (see `docs/research/2026-10-09-release-and-converter-packaging.md`).
- GitHub branch protection and named-actor rulesets must restrict both `master` and `release` updates and release-tag creation to the maintainer, while release tags cannot be updated or deleted. These controls do not cryptographically prove the local gate ran. Repository administrators can edit governance settings, so review ruleset changes as part of repository administration.
