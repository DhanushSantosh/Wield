<!-- Release PR? Use the channel-specific template instead:
     beta-release.md for a PR targeting master, or stable-release.md for a PR
     targeting release. See CONTRIBUTING.md for the selection instructions. -->

## What changed

<!-- Describe the user-visible behavior and link an issue when applicable. -->

## Verification

- [ ] `npm ci && npm run check`
- [ ] `npm audit --audit-level=high` and `cargo audit`
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --locked -- --test-threads=1`
- [ ] Manual runtime check for UI, portal, native, or packaging changes (details below)

Commands and results:

## Release and safety review

- [ ] Version, docs, screenshots, and release notes updated if relevant
- [ ] New converter dependencies have a packaging/license/clean-machine test plan
- [ ] No secrets, generated build outputs, or unrelated changes included

## Risks / rollback

<!-- State relevant failure modes and how to revert safely. -->
