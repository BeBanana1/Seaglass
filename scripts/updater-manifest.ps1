# Desktop updater manifest (release/latest.json), written by release.ps1 and
# release-linux.ps1 (#314): a single function, so that one does not erase the other.
# Load after build-stamp.ps1; uses the script's $root and Fail.
#
# The manifest only describes the given version, with one entry per platform whose build is
# of this version, of this commit, and in the same state (clean or not) as the tree: never
# an entry of an older version, which the updater would install.
#   windows-x86_64 : release/<version>/Seaglass_<version>_x64-setup.exe(.sig)
#   linux-x86_64   : release/<version>/Seaglass_<version>_amd64.AppImage(.sig)

$UpdaterPlatforms = [ordered]@{
    'windows-x86_64' = @{ stamp = 'windows'; file = { param($v) "Seaglass_${v}_x64-setup.exe" } }
    'linux-x86_64'   = @{ stamp = 'linux'; file = { param($v) "Seaglass_${v}_amd64.AppImage" } }
}

# True if the last build of the platform is of this version and of this tree state.
function Test-FreshBuild([string]$platform, [string]$version) {
    $path = Get-BuildStampPath $platform
    if (-not (Test-Path $path)) { return $false }
    $stamp = Get-Content $path -Raw -Encoding UTF8 | ConvertFrom-Json
    $stamp.version -eq $version -and
        $stamp.commit -eq (git rev-parse HEAD) -and
        [bool]$stamp.dirty -eq [bool](git status --porcelain)
}

# Writes release/latest.json; returns the announced platforms.
function Write-UpdaterManifest([string]$version, [string]$notes, [string]$baseUrl) {
    $platforms = [ordered]@{}
    foreach ($entry in $UpdaterPlatforms.GetEnumerator()) {
        if (-not (Test-FreshBuild $entry.Value.stamp $version)) { continue }
        $name = & $entry.Value.file $version
        $sig = Join-Path $root "release\$version\$name.sig"
        if (-not (Test-Path $sig)) { Fail "signature missing for $($entry.Key): $sig" }
        $platforms[$entry.Key] = [ordered]@{
            signature = (Get-Content $sig -Raw).Trim()
            url       = "$baseUrl/$version/$name"
        }
    }
    if ($platforms.Count -eq 0) { Fail "no $version build on this commit for the updater manifest" }
    $manifest = [ordered]@{
        version   = $version
        notes     = $notes
        pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
        platforms = $platforms
    }
    $path = Join-Path $root 'release\latest.json'
    # UTF-8 without BOM: the updater reads strict JSON.
    [System.IO.File]::WriteAllText($path, ($manifest | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))
    @($platforms.Keys)
}

# Before uploading latest.json: every announced file, except those the caller has just
# published ($except), must already be online. Otherwise a platform's updater would look
# for a missing file (Windows-only release after a Linux build of the same commit).
# Local destination (tests): the file is looked up there; otherwise, HEAD request on the URL.
function Assert-ManifestOnline([string]$manifestPath, [string]$dest, [string[]]$except) {
    $manifest = Get-Content $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    # Same rule as publishing: "host:..." (at least 2 characters, not "D:").
    $remote = $dest -match '^[^\\/:]{2,}:'
    foreach ($platform in $manifest.platforms.PSObject.Properties) {
        if ($except -contains $platform.Name) { continue }
        $url = $platform.Value.url
        if ($remote) {
            try {
                $response = Invoke-WebRequest -Uri $url -Method Head -UseBasicParsing -TimeoutSec 30
                $ok = $response.StatusCode -eq 200
            } catch { $ok = $false }
        } else {
            $ok = Test-Path (Join-Path $dest "$($manifest.version)\$(Split-Path $url -Leaf)")
        }
        if (-not $ok) {
            Fail ("latest.json announces $($platform.Name), but $url is not online:`n" +
                "    publish that platform first (release-all.ps1 does it in order)")
        }
        Write-Host "    $($platform.Name) already online"
    }
}
