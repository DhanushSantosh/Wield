# Beta release candidate: vX.Y.Z-beta.N

<!-- Use this template for a release-candidate PR targeting master. Replace examples and leave unmet checks unchecked. Do not push a v* tag from this PR branch. -->

## Identity and scope

- Target branch: `master`
- Proposed tag: `vX.Y.Z-beta.N`
- Previous beta/stable tag and included PRs:
- User-visible changes, breaking changes, and known limitations:
- Release notes and converter prerequisites:

## Before merge

- [ ] The tag follows `vX.Y.Z-beta.N` (`N` starts at 1 and increases for this version); it is new and has not been pushed.
- [ ] The beta version matches `apps/wield/package.json`, `apps/wield/src-tauri/tauri.conf.json`, `package-lock.json`, all six crate manifests, and `Cargo.lock` is updated.
- [ ] `node scripts/release-check.mjs manifest vX.Y.Z-beta.N` passes with the proposed tag.
- [ ] `npm ci`, `npm audit --audit-level=high`, and `cargo audit` pass; disclose any advisory warnings.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked -- --test-threads=1`, and `npm run check` pass.
- [ ] A real desktop smoke test covers changed UI, portal, native, and conversion behavior; record distribution/session, commands, and results below.
- [ ] Release notes disclose host-installed converter requirements, unsupported capabilities, and any new bundled binary's license/notice obligations.
- [ ] The `test` CI check is green; the required review or documented maintainer admin bypass is resolved. Do not bypass a failing check.

Verification evidence (commit SHA, host, commands, results, screenshots/log links):

## After merge — before tag push

<!-- Record these results in a PR comment if the PR body can no longer be edited. The beta tag must point to the merged master commit, not the PR head. -->

- [ ] Record the merged `master` SHA and confirm the working tree is clean and `HEAD` is on `origin/master`.
- [ ] Run `bash scripts/release-local.sh vX.Y.Z-beta.N` on that exact SHA; record the full result and AppImage checksum.
- [ ] Re-run the local gate if the source, version, lockfile, or release scripts changed after the recorded pass.
- [ ] Only after local success, create and push an annotated tag on that SHA. Do not move or reuse a `v*` tag.
- [ ] GitHub release CI passes and creates a draft with one AppImage and `SHA256SUMS`; verify the downloaded assets.
- [ ] Review notes, architecture, licenses, and clean-machine launch/conversions before publishing; publish as **prerelease**, with **latest disabled**.

Tag SHA, local-gate result, draft URL, and publication decision:

## Risks and recovery

<!-- Describe beta-specific risks and how users recover. A bad immutable tag or published release requires a new version, not a rewritten tag or asset. -->
