#!/usr/bin/env bash
# Checks that `VERSION` changed if the shipped app changed since the last release.
# Run by CI (full history and tags required: `fetch-depth: 0`); usable locally before a
# PR: `bash scripts/check-version.sh`.
#
# Fails if:
#   - `VERSION` is lower than the last release (closest `v*` tag);
#   - `VERSION` still equals the last release while app files changed since that tag.
#     Otherwise installs would not see the update (same number) and `main` would no
#     longer match the published version.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

version=$(tr -d '[:space:]' < VERSION)
tag=$(git describe --tags --abbrev=0 --match 'v[0-9]*' HEAD 2>/dev/null || true)
if [[ -z "$tag" ]]; then
  echo "No release tag in the history: nothing to compare."
  exit 0
fi
released=${tag#v}

if [[ "$version" != "$released" ]]; then
  newest=$(printf '%s\n%s\n' "$version" "$released" | sort -V | tail -n1)
  if [[ "$newest" != "$version" ]]; then
    echo "::error::VERSION ($version) is lower than the last release ($tag)."
    exit 1
  fi
  echo "VERSION $version > last release $released: OK."
  exit 0
fi

# What does not change the shipped app: documentation (including the README screenshots
# in `docs/images/`), CI, tooling, mock backend (removed from the production build),
# frontend tests (#264, outside the bundle), Jam relay (deployed separately on the server),
# project website (#517, published by scripts/publish-site.ps1).
changed=$(git diff --name-only "$tag" HEAD -- . \
  ':(exclude,glob)**/*.md' \
  ':(exclude)docs/' \
  ':(exclude)LICENSE' \
  ':(exclude).env.example' \
  ':(exclude).github/' \
  ':(exclude)scripts/' \
  ':(exclude)spike/' \
  ':(exclude)relay/' \
  ':(exclude)site/' \
  ':(exclude).claude/' \
  ':(exclude).mcp.json' \
  ':(exclude).gitignore' \
  ':(exclude).gitattributes' \
  ':(exclude).git-blame-ignore-revs' \
  ':(exclude)src/lib/mock/' \
  ':(exclude,glob)src/**/*.test.ts' \
  ':(exclude)src/lib/testing/')

if [[ -n "$changed" ]]; then
  echo "::error::The app changed since $tag but VERSION is still $version: move to the next version (VERSION and src-tauri/Cargo.toml)."
  echo "Files changed since $tag:"
  echo "$changed" | sed 's/^/  /'
  exit 1
fi
echo "VERSION $version = last release, no app change since $tag: OK."
