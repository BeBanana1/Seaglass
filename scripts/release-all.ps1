<#
.SYNOPSIS
  Builds then publishes Seaglass for Windows, Android and Linux, on the same commit.

.DESCRIPTION
  1. Checks the git tree and the `v<version>` tag (absent, or already on HEAD).
  2. Builds the Windows installer (`release.ps1`), the APK (`release-android.ps1`), then
     the AppImage (`release-linux.ps1`, in WSL), without publishing anything: if a build
     fails, nothing is online.
  3. With -Publish: publishes Android, Linux then Windows (`-PublishOnly`: the files of the
     step 2 build, checked through their stamp), sets the tag, then creates the GitHub
     release with the installer, the APK and the AppImage (`github-release.ps1`). Windows
     goes last: it uploads latest.json, which also announces the AppImage to the updater
     (#314).

  -SkipLinux leaves Linux out (WSL or its build distribution missing).

  Keys, passwords and environment variables: those of release.ps1, release-android.ps1,
  release-linux.ps1 and github-release.ps1. Each step runs in its own PowerShell process;
  the first one that fails stops everything.

.EXAMPLE
  powershell -File scripts/release-all.ps1            # builds everything, publishes nothing
  powershell -File scripts/release-all.ps1 -Publish   # builds, publishes, tags, GitHub release
  powershell -File scripts/release-all.ps1 -Publish -Notes "- Playback fix"
#>
param(
    [switch]$Publish,
    [string]$Notes,
    # Without the Linux AppImage (WSL or its build distribution missing).
    [switch]$SkipLinux
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Fail($message) { Write-Host "ERROR: $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "`n=== $message ===" -ForegroundColor Magenta }

# Each script in its own process: its `exit` and its environment variables do not affect
# this one. Called directly (not in a function) so that their output goes to the console
# without being captured, colors included; exit code in $LASTEXITCODE.
$ps = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File')
$windows = Join-Path $PSScriptRoot 'release.ps1'
$android = Join-Path $PSScriptRoot 'release-android.ps1'
$linux = Join-Path $PSScriptRoot 'release-linux.ps1'
$github = Join-Path $PSScriptRoot 'github-release.ps1'

# --- 1. Checks -------------------------------------------------------------------
# Repeated by each script; done here to fail before the builds.
$version = (Get-Content VERSION -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { Fail "invalid VERSION: '$version'" }
$tag = "v$version"
if (git status --porcelain) { Fail "there are uncommitted changes" }
if ($Publish) {
    if (-not $env:SEAGLASS_RELEASE_DEST) { Fail "SEAGLASS_RELEASE_DEST is not set" }
    if ((git tag --list $tag) -and (git rev-parse "$tag^{commit}") -ne (git rev-parse HEAD)) {
        Fail "tag $tag exists on another commit: bump VERSION before publishing"
    }
}
$buildArgs = if ($Notes) { @('-Notes', $Notes) } else { @() }

# --- 2. Builds --------------------------------------------------------------------------
Step "Windows ${version}: build"
powershell @ps $windows @buildArgs
if ($LASTEXITCODE -ne 0) { Fail "Windows build failed, nothing was published" }
Step "Android ${version}: build"
powershell @ps $android @buildArgs
if ($LASTEXITCODE -ne 0) { Fail "Android build failed, nothing was published" }
if ($SkipLinux) {
    Write-Host "`nLinux left out (-SkipLinux)" -ForegroundColor Yellow
} else {
    Step "Linux ${version}: build"
    powershell @ps $linux @buildArgs
    if ($LASTEXITCODE -ne 0) { Fail "Linux build failed, nothing was published (or rerun with -SkipLinux)" }
}
$platforms = if ($SkipLinux) { 'Windows and Android' } else { 'Windows, Android and Linux' }

if (-not $Publish) {
    Write-Host "`n$platforms builds done, nothing was published (add -Publish)." -ForegroundColor Green
    exit 0
}

# --- 3. Publishing ---------------------------------------------------------------------
# Windows last: its publication uploads latest.json, which also announces the AppImage
# (#314); it checks that every announced file is already online.
Step "Android ${version}: publishing"
powershell @ps $android -PublishOnly -SkipGitHubRelease
if ($LASTEXITCODE -ne 0) {
    Fail "Android publishing failed, nothing else was published (rerun: powershell -File scripts/release-all.ps1 -Publish)"
}
if (-not $SkipLinux) {
    Step "Linux ${version}: publishing"
    powershell @ps $linux -PublishOnly -SkipGitHubRelease
    if ($LASTEXITCODE -ne 0) {
        Fail ("Android is published but neither Linux nor Windows. To finish:`n" +
            "    powershell -File scripts/release-linux.ps1 -PublishOnly`n" +
            "    powershell -File scripts/release.ps1 -PublishOnly")
    }
}
Step "Windows ${version}: publishing (latest.json last)"
powershell @ps $windows -PublishOnly -SkipGitHubRelease
if ($LASTEXITCODE -ne 0) {
    Fail ("Android$(if (-not $SkipLinux) { ' and Linux are published' } else { ' is published' }), but not Windows nor the updater manifest. To finish:`n" +
        "    powershell -File scripts/release.ps1 -PublishOnly")
}

# Everything is online: a failure here is only a warning (#108).
Step "GitHub release $tag"
powershell @ps $github -Version $version
if ($LASTEXITCODE -ne 0) {
    Write-Host ("WARNING: GitHub release not created, the updates stay published.`n" +
        "    To redo it alone: powershell -File scripts/github-release.ps1") -ForegroundColor Yellow
}
Write-Host "`nSeaglass $version published for $platforms." -ForegroundColor Green
