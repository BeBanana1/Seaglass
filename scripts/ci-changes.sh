#!/usr/bin/env bash
# Which CI jobs a change needs (#24): prints `app=true|false`, `site=…`, `relay=…`, one per
# line, for `$GITHUB_OUTPUT`. Usage: `bash scripts/ci-changes.sh <base-commit> [<head>]`.
# Without a base (push to main, scheduled or manual run): everything runs.
#
# - app: anything that the `linux` / `windows` jobs check (Rust, frontend, its tests and mock
#   backend, dependencies). Conservative: a file is "app" unless it is in the list below.
# - site: `site/`. relay: `relay/`.
# - The CI itself (this script, the workflow, the version check) runs everything.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

all() { printf 'app=true\nsite=true\nrelay=true\n'; }

base=${1:-}
head=${2:-HEAD}
if [[ -z "$base" ]] || ! git cat-file -e "$base^{commit}" 2>/dev/null; then
  all
  exit 0
fi

files=$(git diff --name-only "$base" "$head")

if grep -qxE '\.github/workflows/ci\.yml|scripts/ci-changes\.sh|scripts/check-version\.sh' <<<"$files"; then
  all
  exit 0
fi

has() { grep -qE "$1" <<<"$files" && echo true || echo false; }

# Not checked by the app jobs: documentation, website, relay, other CI and repository
# files, release scripts (PowerShell, never run in CI), spike, editor settings.
not_app='^(site/|relay/|docs/|spike/|scripts/|\.claude/|\.github/)|\.md$|^(LICENSE|\.env\.example|\.mcp\.json|\.gitignore|\.gitattributes|\.git-blame-ignore-revs|\.editorconfig)$'
if grep -qvE "$not_app" <<<"$files"; then app=true; else app=false; fi

printf 'app=%s\nsite=%s\nrelay=%s\n' "$app" "$(has '^site/')" "$(has '^relay/')"
