# Repère du dernier build d'une plateforme (release.ps1, release-android.ps1), pour que
# -PublishOnly ne publie que des fichiers construits sur ce commit, arbre propre.
# Gardé dans release/.build/ : hors des dossiers de version, jamais publié.
# À charger avec « . (Join-Path $PSScriptRoot 'build-stamp.ps1') » ; utilise Fail du script.

function Get-BuildStampPath([string]$platform) {
    Join-Path $root "release\.build\$platform.json"
}

function Write-BuildStamp([string]$platform, [string]$version) {
    $path = Get-BuildStampPath $platform
    New-Item -ItemType Directory -Force (Split-Path $path) | Out-Null
    $stamp = [ordered]@{
        version = $version
        commit  = (git rev-parse HEAD)
        dirty   = [bool](git status --porcelain)
    }
    [System.IO.File]::WriteAllText($path, ($stamp | ConvertTo-Json), [System.Text.UTF8Encoding]::new($false))
}

function Assert-BuildStamp([string]$platform, [string]$version, [string[]]$files) {
    $path = Get-BuildStampPath $platform
    if (-not (Test-Path $path)) { Fail "aucun build $platform à publier (lancer d'abord le build sans -PublishOnly)" }
    $stamp = Get-Content $path -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($stamp.version -ne $version) { Fail "le dernier build $platform est la $($stamp.version), pas la $version" }
    if ($stamp.commit -ne (git rev-parse HEAD)) { Fail "le dernier build $platform a été fait sur un autre commit ($($stamp.commit))" }
    if ($stamp.dirty) { Fail "le dernier build $platform a été fait avec des modifications non commitées" }
    foreach ($file in $files) {
        if (-not (Test-Path $file)) { Fail "fichier du build $platform absent : $file" }
    }
}
