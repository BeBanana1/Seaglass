<#
.SYNOPSIS
  Builds and publishes the project website (site/, #517, Astro + Starlight since #18).
.DESCRIPTION
  Builds the site (`pnpm build` in site/: copies the README screenshots and the icon, runs
  Astro, then moves the inline code out of the pages for the server's CSP), then copies
  site/dist/ to SEAGLASS_SITE_DEST. Nothing depends on a release: the download page reads the
  manifests of the updates folder (/updates/windows.json, android.json, linux.json) written by
  the release scripts.
  Assets (scripts, styles, images: hashed names under /_astro/) are copied before the pages
  that load them, so a page never goes online before what it needs. Files of older builds are
  left in place (unreferenced hashed assets, harmless). On a scp target, the permissions are
  reset to 755 / 644 afterwards (Windows permissions are too wide).
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
$dist = Join-Path $site 'dist'

Step "build"
pnpm --dir $site install --frozen-lockfile
if ($LASTEXITCODE -ne 0) { Fail "pnpm install failed in site/" }
pnpm --dir $site build
if ($LASTEXITCODE -ne 0) { Fail "the website build failed" }

# Relative paths with forward slashes, assets first, pages (*.html) last.
$all = Get-ChildItem $dist -Recurse -File | ForEach-Object {
    $_.FullName.Substring($dist.Length + 1).Replace('\', '/')
}
$assets = @($all | Where-Object { $_ -notlike '*.html' })
$pages = @($all | Where-Object { $_ -like '*.html' })
if (-not ($pages -contains 'index.html') -or -not ($pages -contains 'download/index.html')) {
    Fail "the build has no home or download page"
}

Step "website -> $dest"
# scp target: "host:..." or "user@host:..." (at least 2 characters before ":", so as not to
# mistake a drive letter such as "D:" for a host).
if ($dest -match '^[^\\/:]{2,}:') {
    $sshHost, $remotePath = $dest -split ':', 2
    # One tar stream per group over a single ssh connection each. Through cmd: Windows
    # PowerShell 5.1 would re-encode the binary stream between two native programs.
    foreach ($group in @($assets, $pages)) {
        $list = Join-Path $env:TEMP 'seaglass-site-files.txt'
        [IO.File]::WriteAllLines($list, [string[]]$group)
        cmd /c "tar -C `"$dist`" -cf - -T `"$list`" | ssh $sshHost `"mkdir -p '$remotePath' && tar -C '$remotePath' -xf -`""
        $code = $LASTEXITCODE
        Remove-Item $list
        if ($code -ne 0) { Fail "copy to the server failed" }
    }
    ssh $sshHost "find '$remotePath' -type d -exec chmod 755 {} + && find '$remotePath' -type f -exec chmod 644 {} +"
    if ($LASTEXITCODE -ne 0) { Fail "could not set the permissions on the server" }
} else {
    foreach ($file in $assets + $pages) {
        $target = Join-Path $dest $file
        New-Item -ItemType Directory -Force (Split-Path -Parent $target) | Out-Null
        Copy-Item (Join-Path $dist $file) $target -Force
    }
}

Write-Host "Website published ($($all.Count) files)." -ForegroundColor Green
