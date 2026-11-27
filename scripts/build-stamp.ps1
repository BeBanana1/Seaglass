# Stamp of the last build of a platform (release.ps1, release-android.ps1), so that
# -PublishOnly only publishes files built on this commit, with a clean tree.
# Kept in release/.build/: outside the version folders, never published.
# Load with ". (Join-Path $PSScriptRoot 'build-stamp.ps1')"; uses the script's Fail.

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
    if (-not (Test-Path $path)) { Fail "no $platform build to publish (run the build first, without -PublishOnly)" }
    $stamp = Get-Content $path -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($stamp.version -ne $version) { Fail "the last $platform build is $($stamp.version), not $version" }
    if ($stamp.commit -ne (git rev-parse HEAD)) { Fail "the last $platform build was made on another commit ($($stamp.commit))" }
    if ($stamp.dirty) { Fail "the last $platform build was made with uncommitted changes" }
    foreach ($file in $files) {
        if (-not (Test-Path $file)) { Fail "$platform build file missing: $file" }
    }
}
