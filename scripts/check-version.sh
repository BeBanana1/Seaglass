#!/usr/bin/env bash
# Vérifie que `VERSION` a changé si l'app livrée a changé depuis la dernière release.
# Lancé par la CI (historique complet et tags requis : `fetch-depth: 0`) ; utilisable en
# local avant une PR : `bash scripts/check-version.sh`.
#
# Échoue si :
#   - `VERSION` est inférieure à la dernière release (tag `v*` le plus proche) ;
#   - `VERSION` vaut encore celle de la dernière release alors que des fichiers de l'app
#     ont changé depuis ce tag. Sinon les installations ne verraient pas la mise à jour
#     (même numéro) et `main` ne correspondrait plus à la version publiée.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

version=$(tr -d '[:space:]' < VERSION)
tag=$(git describe --tags --abbrev=0 --match 'v[0-9]*' HEAD 2>/dev/null || true)
if [[ -z "$tag" ]]; then
  echo "Aucun tag de release dans l'historique : rien à comparer."
  exit 0
fi
released=${tag#v}

if [[ "$version" != "$released" ]]; then
  newest=$(printf '%s\n%s\n' "$version" "$released" | sort -V | tail -n1)
  if [[ "$newest" != "$version" ]]; then
    echo "::error::VERSION ($version) est inférieure à la dernière release ($tag)."
    exit 1
  fi
  echo "VERSION $version > dernière release $released : OK."
  exit 0
fi

# Ce qui ne change pas l'app livrée : documentation, CI, outillage, backend factice
# (éliminé du build de production), tests du frontend (#264, hors du bundle), relais de la
# Jam (déployé à part sur le serveur).
changed=$(git diff --name-only "$tag" HEAD -- . \
  ':(exclude,glob)**/*.md' \
  ':(exclude)LICENSE' \
  ':(exclude).gitea/' \
  ':(exclude)ci/' \
  ':(exclude)scripts/' \
  ':(exclude)spike/' \
  ':(exclude)relay/' \
  ':(exclude).claude/' \
  ':(exclude).mcp.json' \
  ':(exclude).gitignore' \
  ':(exclude).gitattributes' \
  ':(exclude)src/lib/mock/' \
  ':(exclude,glob)src/**/*.test.ts' \
  ':(exclude)src/lib/testing/')

if [[ -n "$changed" ]]; then
  echo "::error::L'app a changé depuis $tag mais VERSION vaut encore $version : passer à la version suivante (VERSION et src-tauri/Cargo.toml)."
  echo "Fichiers modifiés depuis $tag :"
  echo "$changed" | sed 's/^/  /'
  exit 1
fi
echo "VERSION $version = dernière release, sans changement de l'app depuis $tag : OK."
