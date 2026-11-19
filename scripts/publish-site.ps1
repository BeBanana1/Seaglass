<#
.SYNOPSIS
  Publishes the project website (site/, #517).

.DESCRIPTION
  Copies the static pages of site/, the README screenshots (docs/images/*.webp, served at
  /images/) and the icon (/favicon.png) to SEAGLASS_SITE_DEST. Nothing is built and nothing
  depends on a release: the download page reads the manifests of the updates folder
  (/updates/windows.json, android.json, linux.json) written by the release scripts.

  Styles and scripts are copied before the pages that load them. On a scp target, the
  permissions are reset to 755 / 644 afterwards (scp keeps Windows permissions that are too
  wide).

  Settings (environment, then .env at the root):
    SEAGLASS_SITE_DEST   destination: local folder or UNC, or scp target
                         "user@host:/path" or "ssh-alias:/path"

.EXAMPLE
  powershell -File scripts/publish-site.ps1
#>
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Fail($message) { Write-Host "ERROR: $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }
. (Join-Path $PSScriptRoot 'release-env.ps1')

$dest = Get-ReleaseSetting 'SEAGLASS_SITE_DEST'
if (-not $dest) { Fail "SEAGLASS_SITE_DEST is not set (environment or .env)" }
$dest = $dest.TrimEnd('/', '\')

$site = Join-Path $root 'site'
# Assets first, pages last: a page never goes online before what it loads.
$assets = @(
    @{ From = (Join-Path $site 'site.css'); To = 'site.css' },
    @{ From = (Join-Path $site 'download.js'); To = 'download.js' },
    @{ From = (Join-Path $root 'src-tauri\icons\64x64.png'); To = 'favicon.png' }
)
foreach ($image in Get-ChildItem (Join-Path $root 'docs\images') -Filter *.webp) {
    $assets += @{ From = $image.FullName; To = "images/$($image.Name)" }
}
$pages = @(
    @{ From = (Join-Path $site 'download\index.html'); To = 'download/index.html' },
    @{ From = (Join-Path $site 'index.html'); To = 'index.html' }
)
$files = $assets + $pages
foreach ($file in $files) {
    if (-not (Test-Path $file.From)) { Fail "missing file: $($file.From)" }
}

Step "website -> $dest"
# scp target: "host:..." or "user@host:..." (at least 2 characters before ":", so as not to
# mistake a drive letter such as "D:" for a host).
if ($dest -match '^[^\\/:]{2,}:') {
    $sshHost, $remotePath = $dest -split ':', 2
    ssh $sshHost "mkdir -p '$remotePath/images' '$remotePath/download'"
    if ($LASTEXITCODE -ne 0) { Fail "could not create the folders on the server" }
    foreach ($file in $files) {
        scp $file.From "$dest/$($file.To)"
        if ($LASTEXITCODE -ne 0) { Fail "scp failed: $($file.To)" }
    }
    $paths = ($files | ForEach-Object { "'$remotePath/$($_.To)'" }) -join ' '
    ssh $sshHost "chmod 755 '$remotePath' '$remotePath/images' '$remotePath/download' && chmod 644 $paths"
    if ($LASTEXITCODE -ne 0) { Fail "could not set the permissions on the server" }
} else {
    foreach ($file in $files) {
        $target = Join-Path $dest $file.To
        New-Item -ItemType Directory -Force (Split-Path -Parent $target) | Out-Null
        Copy-Item $file.From $target -Force
    }
}
Write-Host "Website published ($($files.Count) files)." -ForegroundColor Green
