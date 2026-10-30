<#
.SYNOPSIS
  Construit, signe et (sur demande) publie une version de Seaglass.

.DESCRIPTION
  1. Vérifie l'arbre git, `.env`, la clé de signature et la version (`VERSION`).
  2. Copie mpv dans `src-tauri/binaries/` (embarqué via `tauri.release.conf.json`).
  3. Construit l'installeur NSIS signé pour l'updater (`pnpm tauri build`).
  4. Range les fichiers à publier dans `release/` :
       release/latest.json                       manifeste lu par l'updater
       release/index.html                        page de téléchargement (lien stable)
       release/<version>/Seaglass_<version>_x64-setup.exe
  5. Avec -Publish : copie `release/` vers SEAGLASS_RELEASE_DEST, puis pose et pousse le
     tag git `v<version>`.

  Variables d'environnement :
    SEAGLASS_UPDATE_BASE_URL  URL publique du dossier de mises à jour
                              (défaut : https://updates.bebanana.ch/seaglass)
    SEAGLASS_RELEASE_DEST     destination de publication : dossier local ou UNC, ou cible
                              scp « utilisateur@hôte:/chemin » ou « alias-ssh:/chemin »
    SEAGLASS_MPV_PATH         mpv.exe à embarquer (défaut : celui du PATH)
    SEAGLASS_SIGNING_KEY      clé privée de l'updater (défaut : ~/.tauri/seaglass.key)

.EXAMPLE
  pnpm release                      # construit dans release/, ne publie rien
  pnpm release -Publish             # construit, publie et tague
  pnpm release -Notes "- Correctif lecture"
#>
param(
    [switch]$Publish,
    [string]$Notes,
    # Pour tester un build avec des modifications non commitées (refusé avec -Publish).
    [switch]$AllowDirty
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Fail($message) { Write-Host "ERREUR : $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }

# --- 1. Vérifications -------------------------------------------------------------------
$version = (Get-Content VERSION -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { Fail "VERSION invalide : '$version'" }
$tag = "v$version"

$dirty = git status --porcelain
if ($dirty -and ($Publish -or -not $AllowDirty)) {
    Fail "des modifications ne sont pas commitées (utiliser -AllowDirty pour un build de test)"
}
if ($Publish) {
    if (git tag --list $tag) { Fail "le tag $tag existe déjà : monter VERSION avant de publier" }
    if (-not $env:SEAGLASS_RELEASE_DEST) { Fail "SEAGLASS_RELEASE_DEST n'est pas défini" }
}
if (-not (Test-Path .env)) { Fail ".env absent : l'identifiant client est injecté à la compilation" }

$keyPath = if ($env:SEAGLASS_SIGNING_KEY) { $env:SEAGLASS_SIGNING_KEY } else { Join-Path $HOME '.tauri\seaglass.key' }
if (-not (Test-Path $keyPath)) { Fail "clé de signature introuvable : $keyPath" }

$baseUrl = if ($env:SEAGLASS_UPDATE_BASE_URL) { $env:SEAGLASS_UPDATE_BASE_URL.TrimEnd('/') } else { 'https://updates.bebanana.ch/seaglass' }

# --- 2. mpv embarqué --------------------------------------------------------------------
Step "mpv"
$mpvSource = if ($env:SEAGLASS_MPV_PATH) { $env:SEAGLASS_MPV_PATH } else { (Get-Command mpv.exe -ErrorAction SilentlyContinue).Source }
if (-not $mpvSource -or -not (Test-Path $mpvSource)) { Fail "mpv.exe introuvable (PATH ou SEAGLASS_MPV_PATH)" }
$sidecar = 'src-tauri\binaries\mpv-x86_64-pc-windows-msvc.exe'
New-Item -ItemType Directory -Force (Split-Path $sidecar) | Out-Null
Copy-Item $mpvSource $sidecar -Force
$mpvVersion = (& $mpvSource --version | Select-Object -First 1)
Write-Host "    $mpvVersion"

# --- 3. Build signé ---------------------------------------------------------------------
Step "build de Seaglass $version"
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
if (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '' }
pnpm tauri build --config (Join-Path $root 'src-tauri\tauri.release.conf.json')
if ($LASTEXITCODE -ne 0) { Fail "échec du build" }

$bundle = 'src-tauri\target\release\bundle\nsis'
$setupName = "Seaglass_${version}_x64-setup.exe"
$setup = Join-Path $bundle $setupName
if (-not (Test-Path $setup) -or -not (Test-Path "$setup.sig")) { Fail "installeur ou signature absents dans $bundle" }

# --- 4. Fichiers à publier --------------------------------------------------------------
Step "manifeste"
$out = Join-Path $root "release\$version"
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item $setup $out -Force

if (-not $Notes) {
    # Pas de `git describe` : sans tag, il écrit sur stderr, ce que PowerShell 5.1 traite
    # comme une erreur fatale avec $ErrorActionPreference = 'Stop'.
    $previous = git tag --list 'v*' --sort=-v:refname | Select-Object -First 1
    $range = if ($previous) { "$previous..HEAD" } else { 'HEAD' }
    $Notes = (git log $range --no-merges --pretty=format:'- %s' -n 30) -join "`n"
}
$manifest = [ordered]@{
    version   = $version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = (Get-Content "$setup.sig" -Raw).Trim()
            url       = "$baseUrl/$version/$setupName"
        }
    }
}
$manifestPath = Join-Path $root 'release\latest.json'
# UTF-8 sans BOM : l'updater lit du JSON strict.
[System.IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))

$size = [math]::Round((Get-Item $setup).Length / 1MB, 1)

# Page de téléchargement : l'adresse du dossier ($baseUrl/) mène toujours à la dernière version.
Step "page de téléchargement"
function Html($text) { [System.Net.WebUtility]::HtmlEncode($text) }
$fr = [System.Globalization.CultureInfo]::GetCultureInfo('fr-FR')
$noteItems = ($Notes -split "`r?`n" | Where-Object { $_.Trim() } | ForEach-Object {
    '      <li>' + (Html ($_ -replace '^\s*-\s*', '')) + '</li>'
}) -join "`n"
$fields = [ordered]@{
    '{{VERSION}}' = Html $version
    '{{DATE}}'    = Html (Get-Date).ToString('d MMMM yyyy', $fr)
    '{{SIZE}}'    = Html $size.ToString($fr)
    '{{URL}}'     = Html "$baseUrl/$version/$setupName"
    '{{NOTES}}'   = $noteItems
}
$page = Get-Content (Join-Path $PSScriptRoot 'download-page.html') -Raw -Encoding UTF8
foreach ($field in $fields.GetEnumerator()) { $page = $page.Replace($field.Key, $field.Value) }
$pagePath = Join-Path $root 'release\index.html'
[System.IO.File]::WriteAllText($pagePath, $page, [System.Text.UTF8Encoding]::new($false))

Write-Host "    installeur : release\$version\$setupName ($size Mo)"
Write-Host "    manifeste  : release\latest.json -> $baseUrl/latest.json"
Write-Host "    page       : release\index.html -> $baseUrl/"

# --- 5. Publication ---------------------------------------------------------------------
if (-not $Publish) {
    Write-Host "Build terminé, rien n'a été publié (ajouter -Publish)." -ForegroundColor Green
    exit 0
}

Step "publication vers $env:SEAGLASS_RELEASE_DEST"
$dest = $env:SEAGLASS_RELEASE_DEST
# Le manifeste part en dernier : l'updater ne voit la version qu'une fois l'installeur en place.
# Cible scp : « hôte:… » ou « utilisateur@hôte:… » (au moins 2 caractères avant « : »,
# pour ne pas confondre avec une lettre de lecteur comme « D: »).
if ($dest -match '^[^\\/:]{2,}:') {
    $sshHost, $remotePath = $dest -split ':', 2
    scp -r $out "$dest/"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (installeur)" }
    # scp reprend des droits Windows trop larges (dossier en écriture pour tous) : on les
    # remet à 755 / 644 avant de rendre la version visible.
    ssh $sshHost "chmod 755 '$remotePath/$version' && chmod 644 '$remotePath/$version'/*"
    if ($LASTEXITCODE -ne 0) { Fail "échec du réglage des droits sur le serveur" }
    scp $pagePath "$dest/index.html"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (page de téléchargement)" }
    scp $manifestPath "$dest/latest.json"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (manifeste)" }
    ssh $sshHost "chmod 644 '$remotePath/index.html' '$remotePath/latest.json'"
} else {
    New-Item -ItemType Directory -Force $dest | Out-Null
    Copy-Item $out (Join-Path $dest $version) -Recurse -Force
    Copy-Item $pagePath (Join-Path $dest 'index.html') -Force
    Copy-Item $manifestPath (Join-Path $dest 'latest.json') -Force
}

Step "tag $tag"
git tag -a $tag -m "Seaglass $version"
git push origin $tag
if ($LASTEXITCODE -ne 0) { Fail "échec du push du tag" }
Write-Host "Seaglass $version publiée." -ForegroundColor Green
