<#
.SYNOPSIS
  Builds and publishes the project website (site/, #517, Astro + Starlight since #18).
.DESCRIPTION
  Builds the site (`pnpm build` in site/: copies the README screenshots and the icon, runs
  Astro, then moves the inline code out of the pages for the server's CSP), then copies
  site/dist/ to SEAGLASS_SITE_DEST. Nothing depends on a release: the download page reads the
  manifests of the updates folder (/updates/windows.json, android.json, linux.json) written by
  the release scripts.
  The app demo (/try/, #28) is built after the site, into site/dist/try/: the app's own
  interface and themes with its mock backend (`vite build --mode demo`), from the latest
  published version (highest v* tag) checked out in a temporary git worktree, so the demo
  matches what can be downloaded rather than the work in progress. A tag older than the demo
  mode falls back to the working tree, with a warning.
  Assets (scripts, styles, images: hashed names under /_astro/ and /try/assets/) are copied
  before the pages that load them, so a page never goes online before what it needs. Files of
  older builds are left in place (unreferenced hashed assets, harmless). On a scp target, the
  permissions are reset to 755 / 644 afterwards (Windows permissions are too wide).
  Settings (environment, then .env at the root):
    SEAGLASS_SITE_DEST   destination: local folder or UNC, or scp target
                         "user@host:/path" or "ssh-alias:/path"
.PARAMETER DemoRef
  Git reference the demo is built from, instead of the latest v* tag ("HEAD": the working
  tree as it is, uncommitted changes included).
.PARAMETER NoPublish
  Builds the site and the demo into site/dist/ and stops, for `pnpm --dir site preview`.
.EXAMPLE
  powershell -File scripts/publish-site.ps1
.EXAMPLE
  powershell -File scripts/publish-site.ps1 -NoPublish -DemoRef HEAD
#>
param([string]$DemoRef, [switch]$NoPublish)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Fail($message) { Write-Host "ERROR: $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }

. (Join-Path $PSScriptRoot 'release-env.ps1')

$dest = Get-ReleaseSetting 'SEAGLASS_SITE_DEST'
if (-not $dest -and -not $NoPublish) { Fail "SEAGLASS_SITE_DEST is not set (environment or .env)" }
if ($dest) { $dest = $dest.TrimEnd('/', '\') }
$site = Join-Path $root 'site'
$dist = Join-Path $site 'dist'

# The demo build of the app (#28) into $out, from $ref ("" = the working tree). The command
# line lives here rather than in a package.json script, so an older tag only needs to know
# `--mode demo`.
function Build-Demo($ref, $out) {
    $tree = $root
    if ($ref) {
        $tree = Join-Path $env:TEMP "seaglass-demo-$PID"
        git worktree add --detach $tree $ref
        if ($LASTEXITCODE -ne 0) { Fail "could not check out $ref for the demo" }
    }
    try {
        pnpm --dir $tree install --frozen-lockfile
        if ($LASTEXITCODE -ne 0) { Fail "pnpm install failed for the demo" }
        pnpm --dir $tree exec vite build --mode demo --base /try/ --outDir $out --emptyOutDir
        if ($LASTEXITCODE -ne 0) { Fail "the demo build failed" }
    } finally {
        if ($ref) { git worktree remove --force $tree }
    }
}

Step "build"
pnpm --dir $site install --frozen-lockfile
if ($LASTEXITCODE -ne 0) { Fail "pnpm install failed in site/" }
pnpm --dir $site build
if ($LASTEXITCODE -ne 0) { Fail "the website build failed" }

if (-not $DemoRef) {
    $DemoRef = git tag --list 'v*' --sort=-v:refname | Select-Object -First 1
    if (-not $DemoRef) { Fail "no v* tag: pass -DemoRef" }
}
$demoRef = $DemoRef
if ($demoRef -eq 'HEAD') {
    $demoRef = ''
} elseif (-not (git ls-tree --name-only $demoRef -- src/lib/mock/demo.ts)) {
    Write-Host "WARNING: $DemoRef has no demo mode yet (#28): the demo is built from the working tree" -ForegroundColor Yellow
    $demoRef = ''
}
Step "demo from $(if ($demoRef) { $demoRef } else { 'the working tree' })"
$try = Join-Path $dist 'try'
Build-Demo $demoRef $try
# The demo must never carry the maintainer's fixtures (account library, real catalog): their
# covers all come from this host, the invented catalog never does.
if (Get-ChildItem $try -Recurse -File | Select-String -Pattern 'resources.tidal.com' -SimpleMatch -List) {
    Fail "the demo build contains catalog data from the fixtures"
}

if ($NoPublish) {
    Write-Host "Site and demo built in $dist (pnpm --dir site preview)." -ForegroundColor Green
    exit 0
}

# Relative paths with forward slashes, assets first, pages (*.html) last.
$all = Get-ChildItem $dist -Recurse -File | ForEach-Object {
    $_.FullName.Substring($dist.Length + 1).Replace('\', '/')
}
$assets = @($all | Where-Object { $_ -notlike '*.html' })
$pages = @($all | Where-Object { $_ -like '*.html' })
foreach ($page in 'index.html', 'download/index.html', 'try/index.html') {
    if (-not ($pages -contains $page)) { Fail "the build has no $page" }
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
    ssh -n $sshHost "find '$remotePath' -type d -exec chmod 755 {} + && find '$remotePath' -type f -exec chmod 644 {} +"
    if ($LASTEXITCODE -ne 0) { Fail "could not set the permissions on the server" }
} else {
    foreach ($file in $assets + $pages) {
        $target = Join-Path $dest $file
        New-Item -ItemType Directory -Force (Split-Path -Parent $target) | Out-Null
        Copy-Item (Join-Path $dist $file) $target -Force
    }
}

Write-Host "Website published ($($all.Count) files)." -ForegroundColor Green
