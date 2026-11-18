#!/bin/bash
# Construit l'AppImage dans la distribution WSL de build (#313), appelé par release-linux.ps1.
#
#   linux-build.sh <arbre source (/mnt/…)> <dossier de sortie (/mnt/…)> <glibc maximale> \
#                  <liste des fichiers à copier (/mnt/…)>
#
# Les fichiers de la liste (chemins relatifs à l'arbre source, écrite par release-linux.ps1)
# sont recopiés dans ~/seaglass-release (compiler sur /mnt est très lent), `target/` y est
# gardé d'un build à l'autre. `dist/` (construit sous Windows) et `.env` sont dans la
# liste. Le build échoue si le binaire ou une bibliothèque embarquée exige une glibc
# plus récente que <glibc maximale> : l'AppImage ne démarrerait pas sur les distributions
# annoncées.
set -euo pipefail

src=$1
out=$2
glibc_max=$3
files=$4
work="$HOME/seaglass-release"

# shellcheck disable=SC1091
source "$HOME/.cargo/env"

echo "==> copie de l'arbre dans $work"
# Seuls les fichiers de la liste sont copiés (#418) : chaque fichier lu sur /mnt coûte cher.
# La copie précédente est vidée, sauf `src-tauri/target` ; les dates sont conservées, donc
# cargo ne recompile que ce qui a changé.
mkdir -p "$work/src-tauri"
find "$work" -mindepth 1 -maxdepth 1 ! -name src-tauri -exec rm -rf {} +
find "$work/src-tauri" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
rsync -a --files-from="$files" "$src/" "$work/"
echo "    $(wc -l < "$files") fichiers"

echo "==> build de l'AppImage"
# Frontend déjà construit. L'AppImage est signée pour l'updater (#314) : la clé et son mot
# de passe arrivent par l'environnement (TAURI_SIGNING_PRIVATE_KEY*, transmis par WSLENV),
# jamais par un fichier.
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
    echo "ERREUR : TAURI_SIGNING_PRIVATE_KEY absente (lancer release-linux.ps1)" >&2
    exit 1
fi
if [ -z "${SEAGLASS_UPDATER_PUBKEY:-}" ]; then
    echo "ERREUR : SEAGLASS_UPDATER_PUBKEY absente (lancer release-linux.ps1)" >&2
    exit 1
fi
config=$(mktemp)
trap 'rm -f "$config"' EXIT
# Clé publique (#489) : le CLI vérifie la signature avec `plugins.updater.pubkey`, vide
# dans `tauri.conf.json`. Du base64, sans caractère à échapper en JSON.
printf '{"build":{"beforeBuildCommand":""},"plugins":{"updater":{"pubkey":"%s"}}}\n' \
    "$SEAGLASS_UPDATER_PUBKEY" > "$config"
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

if [ ! -s "${images[0]}.sig" ]; then
    echo "ERREUR : signature de l'updater absente (${images[0]}.sig)" >&2
    exit 1
fi

mkdir -p "$out"
cp "${images[0]}" "${images[0]}.sig" "$out/"
echo "APPIMAGE=$(basename "${images[0]}")"
