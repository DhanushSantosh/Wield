#!/usr/bin/env bash
set -euo pipefail

tag="${1:?Usage: bash scripts/release-local.sh vX.Y.Z[-beta.N]}"
repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"
node scripts/release-check.mjs manifest "$tag"
release_branch="$(node scripts/release-check.mjs branch "$tag")"

# A local pass only means something if it covers exactly the commit that will be tagged.
if [ -n "$(git status --porcelain)" ]; then
  git status --porcelain >&2
  echo "Working tree is not clean; commit, stash, or remove these files first." >&2
  exit 1
fi
git fetch --quiet origin "$release_branch"
head_sha="$(git rev-parse HEAD)"
if ! git merge-base --is-ancestor "$head_sha" "origin/$release_branch"; then
  echo "HEAD $head_sha is not on origin/$release_branch; $tag must point to a merged $release_branch commit." >&2
  exit 1
fi
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  tag_sha="$(git rev-list -n 1 "$tag")"
  if [ "$tag_sha" != "$head_sha" ]; then
    echo "Tag $tag already points to $tag_sha, not HEAD $head_sha." >&2
    exit 1
  fi
fi

docker build --network host --file .github/release.Dockerfile --tag wield-release-local .github
docker run --rm --network host \
  --user "$(id -u):$(id -g)" \
  --env HOME=/tmp \
  --env CARGO_HOME=/workspace/target/ubuntu-cargo-home \
  --env CARGO_TARGET_DIR=/workspace/target/ubuntu-22.04 \
  --volume "$repo_root:/workspace" \
  --workdir /workspace \
  wield-release-local bash scripts/release-gate.sh "$tag"

if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  git status --porcelain --untracked-files=no >&2
  echo "The release gate modified tracked files (e.g. a lockfile); fix that in a PR before tagging." >&2
  exit 1
fi
echo "Local release gate passed for $tag at $head_sha"
