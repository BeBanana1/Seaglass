<#
.SYNOPSIS
  Construit puis publie Seaglass pour Windows et Android, sur le même commit.

.DESCRIPTION
  1. Vérifie l'arbre git et le tag `v<version>` (absent, ou déjà sur HEAD).
  2. Construit l'installeur Windows (`release.ps1`), puis l'APK (`release-android.ps1`),
     sans rien publier : si l'un des deux builds échoue, rien n'est en ligne.
  3. Avec -Publish : publie Windows puis Android (`-PublishOnly` : les fichiers du build
     de l'étape 2, contrôlés par leur repère), pose le tag, puis crée la release Gitea
     avec l'installeur et l'APK (`gitea-release.ps1`).

  Clés, mots de passe et variables d'environnement : ceux de release.ps1,
  release-android.ps1 et gitea-release.ps1. Chaque étape tourne dans son propre
  processus PowerShell ; la première qui échoue arrête tout.

.EXAMPLE
  powershell -File scripts/release-all.ps1            # construit les deux, ne publie rien
  powershell -File scripts/release-all.ps1 -Publish   # construit, publie, tague, release Gitea
  powershell -File scripts/release-all.ps1 -Publish -Notes "- Correctif lecture"
#>
param(
    [switch]$Publish,
    [string]$Notes
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Fail($message) { Write-Host "ERREUR : $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "`n=== $message ===" -ForegroundColor Magenta }

# Chaque script dans un processus à part : ses `exit` et ses variables d'environnement ne
# touchent pas celui-ci. Appelés directement (pas dans une fonction) pour que leur sortie
# aille à la console sans être capturée, couleurs comprises ; code dans $LASTEXITCODE.
$ps = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File')
$windows = Join-Path $PSScriptRoot 'release.ps1'
$android = Join-Path $PSScriptRoot 'release-android.ps1'
$gitea = Join-Path $PSScriptRoot 'gitea-release.ps1'

# --- 1. Vérifications -------------------------------------------------------------------
# Refaites par chaque script ; ici pour échouer avant les builds.
$version = (Get-Content VERSION -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { Fail "VERSION invalide : '$version'" }
$tag = "v$version"
if (git status --porcelain) { Fail "des modifications ne sont pas commitées" }
if ($Publish) {
    if (-not $env:SEAGLASS_RELEASE_DEST) { Fail "SEAGLASS_RELEASE_DEST n'est pas défini" }
    if ((git tag --list $tag) -and (git rev-parse "$tag^{commit}") -ne (git rev-parse HEAD)) {
        Fail "le tag $tag existe sur un autre commit : monter VERSION avant de publier"
    }
}
$buildArgs = if ($Notes) { @('-Notes', $Notes) } else { @() }

# --- 2. Builds --------------------------------------------------------------------------
Step "Windows $version : build"
powershell @ps $windows @buildArgs
if ($LASTEXITCODE -ne 0) { Fail "build Windows en échec, rien n'a été publié" }
Step "Android $version : build"
powershell @ps $android @buildArgs
if ($LASTEXITCODE -ne 0) { Fail "build Android en échec, rien n'a été publié" }

if (-not $Publish) {
    Write-Host "`nBuilds Windows et Android terminés, rien n'a été publié (ajouter -Publish)." -ForegroundColor Green
    exit 0
}

# --- 3. Publication ---------------------------------------------------------------------
Step "Windows $version : publication"
powershell @ps $windows -PublishOnly -SkipGiteaRelease
if ($LASTEXITCODE -ne 0) {
    Fail "publication Windows en échec, Android non publié (relancer : powershell -File scripts/release-all.ps1 -Publish)"
}
Step "Android $version : publication"
powershell @ps $android -PublishOnly -SkipGiteaRelease
if ($LASTEXITCODE -ne 0) {
    Fail ("Windows est publié mais pas Android. Pour finir :`n" +
        "    powershell -File scripts/release-android.ps1 -PublishOnly")
}

# Tout est en ligne : un échec ici n'est qu'un avertissement (#108).
Step "release Gitea $tag"
powershell @ps $gitea -Version $version
if ($LASTEXITCODE -ne 0) {
    Write-Host ("ATTENTION : release Gitea non créée, les mises à jour restent publiées.`n" +
        "    Pour la refaire seule : powershell -File scripts/gitea-release.ps1") -ForegroundColor Yellow
}
Write-Host "`nSeaglass $version publiée pour Windows et Android." -ForegroundColor Green
