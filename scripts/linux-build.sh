#!/bin/bash
# Builds the AppImage in the WSL build distribution (#313), called by release-linux.ps1.
#
#   linux-build.sh <source tree (/mnt/...)> <output folder (/mnt/...)> <maximum glibc> \
#                  <list of files to copy (/mnt/...)>
#
# The files of the list (paths relative to the source tree, written by release-linux.ps1)
# are copied into ~/seaglass-release (compiling on /mnt is very slow), and `target/` is
# kept there from one build to the next. `dist/` (built on Windows) and `.env` are in the
# list. The build fails if the binary or an embedded library requires a newer glibc than
# <maximum glibc>: the AppImage would not start on the announced distributions.
set -euo pipefail

src=$1
out=$2
glibc_max=$3
files=$4
work="$HOME/seaglass-release"

# shellcheck disable=SC1091
source "$HOME/.cargo/env"

echo "==> copying the tree to $work"
# Only the files of the list are copied (#418): every file read from /mnt is expensive.
# The previous copy is emptied, except `src-tauri/target`; timestamps are kept, so cargo
# only recompiles what changed.
mkdir -p "$work/src-tauri"
find "$work" -mindepth 1 -maxdepth 1 ! -name src-tauri -exec rm -rf {} +
find "$work/src-tauri" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
rsync -a --files-from="$files" "$src/" "$work/"
echo "    $(wc -l < "$files") files"

echo "==> building the AppImage"
# Frontend already built. The AppImage is signed for the updater (#314): the key and its
# password come through the environment (TAURI_SIGNING_PRIVATE_KEY*, passed on by WSLENV),
# never through a file.
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
    echo "ERROR: TAURI_SIGNING_PRIVATE_KEY is missing (run release-linux.ps1)" >&2
    exit 1
fi
if [ -z "${SEAGLASS_UPDATER_PUBKEY:-}" ]; then
    echo "ERROR: SEAGLASS_UPDATER_PUBKEY is missing (run release-linux.ps1)" >&2
    exit 1
fi
config=$(mktemp)
trap 'rm -f "$config"' EXIT
# Public key (#489): the CLI checks the signature with `plugins.updater.pubkey`, which is
# empty in `tauri.conf.json`. Base64, with no character to escape in JSON.
printf '{"build":{"beforeBuildCommand":""},"plugins":{"updater":{"pubkey":"%s"}}}\n' \
    "$SEAGLASS_UPDATER_PUBKEY" > "$config"
cd "$work/src-tauri"
rm -rf target/release/bundle/appimage
cargo tauri build --bundles appimage --config "$config"

shopt -s nullglob
images=(target/release/bundle/appimage/*.AppImage)
if [ ${#images[@]} -ne 1 ]; then
    echo "ERROR: one AppImage expected, found ${#images[@]}" >&2
    exit 1
fi

echo "==> glibc required (maximum allowed: $glibc_max)"
appdir=(target/release/bundle/appimage/*.AppDir)
needed=$( { objdump -T target/release/seaglass
            find "${appdir[@]}" -type f \( -name '*.so' -o -name '*.so.*' \) -exec objdump -T {} + 2>/dev/null
          } | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -1)
echo "    $needed"
if [ "$(printf '%s\n%s\n' "$needed" "$glibc_max" | sort -V | tail -1)" != "$glibc_max" ]; then
    echo "ERROR: glibc $needed required, above $glibc_max (built on a distribution that is too recent?)" >&2
    exit 1
fi

if [ ! -s "${images[0]}.sig" ]; then
    echo "ERROR: updater signature missing (${images[0]}.sig)" >&2
    exit 1
fi

mkdir -p "$out"
cp "${images[0]}" "${images[0]}.sig" "$out/"
echo "APPIMAGE=$(basename "${images[0]}")"
