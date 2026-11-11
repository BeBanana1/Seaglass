#!/bin/bash
# Construit l'AppImage dans la distribution WSL de build (#313), appelé par release-linux.ps1.
#
#   linux-build.sh <arbre source (/mnt/…)> <dossier de sortie (/mnt/…)> <glibc maximale>
#
# L'arbre est recopié dans ~/seaglass-release (compiler sur /mnt est très lent), `target/`
# y est gardé d'un build à l'autre. `dist/` (construit sous Windows) et `.env` font partie
# de la copie. Le build échoue si le binaire ou une bibliothèque embarquée exige une glibc
# plus récente que <glibc maximale> : l'AppImage ne démarrerait pas sur les distributions
# annoncées.
set -euo pipefail

src=$1
out=$2
glibc_max=$3
work="$HOME/seaglass-release"

# shellcheck disable=SC1091
source "$HOME/.cargo/env"

echo "==> copie de l'arbre dans $work"
mkdir -p "$work"
rsync -a --delete \
    --exclude /.git --exclude /node_modules --exclude /release --exclude /spike \
    --exclude /src-tauri/target --exclude /src-tauri/gen --exclude /src-tauri/binaries \
    "$src/" "$work/"

echo "==> build de l'AppImage"
# Frontend déjà construit, artefacts de l'updater laissés à #314 (ils exigent la clé).
config=$(mktemp)
trap 'rm -f "$config"' EXIT
cat > "$config" <<'JSON'
{"build":{"beforeBuildCommand":""},"bundle":{"createUpdaterArtifacts":false}}
JSON
cd "$work/src-tauri"
rm -rf target/release/bundle/appimage
cargo tauri build --bundles appimage --config "$config"

shopt -s nullglob
images=(target/release/bundle/appimage/*.AppImage)
if [ ${#images[@]} -ne 1 ]; then
    echo "ERREUR : une AppImage attendue, ${#images[@]} trouvée(s)" >&2
    exit 1
fi

echo "==> glibc exigée (maximum admis : $glibc_max)"
appdir=(target/release/bundle/appimage/*.AppDir)
needed=$( { objdump -T target/release/seaglass
            find "${appdir[@]}" -type f \( -name '*.so' -o -name '*.so.*' \) -exec objdump -T {} + 2>/dev/null
          } | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -1)
echo "    $needed"
if [ "$(printf '%s\n%s\n' "$needed" "$glibc_max" | sort -V | tail -1)" != "$glibc_max" ]; then
    echo "ERREUR : glibc $needed exigée, au-delà de $glibc_max (build sur une distribution trop récente ?)" >&2
    exit 1
fi

mkdir -p "$out"
cp "${images[0]}" "$out/"
echo "APPIMAGE=$(basename "${images[0]}")"
