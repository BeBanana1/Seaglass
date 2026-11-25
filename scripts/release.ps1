<#
.SYNOPSIS
  Builds, signs and (on request) publishes a Seaglass version.

.DESCRIPTION
  1. Checks the git tree, `.env`, the signing key and the version (`VERSION`).
  2. Copies mpv into `src-tauri/binaries/` (bundled through `tauri.release.conf.json`), if it
     matches `scripts/mpv.lock` (version + SHA-256).
  3. Checks the key password, then builds the NSIS installer signed for the updater
     (`pnpm tauri build`).
  4. Puts the files to publish in `release/`:
       release/latest.json                       manifest read by the updater
       release/windows.json                      read by the website's download page (#517)
       release/<version>/Seaglass_<version>_x64-setup.exe
  5. With -Publish: copies `release/` to SEAGLASS_RELEASE_DEST, sets and pushes the git tag
     `v<version>` (unless it is already on HEAD: Android release), then creates the GitHub
     release of the tag (`github-release.ps1`, unless -SkipGitHubRelease).

  -PublishOnly skips steps 2 to 4 and publishes the last build, if it was made on this
  commit with a clean tree (`release-all.ps1` builds Windows and Android before publishing).

  Environment variables:
    SEAGLASS_UPDATE_BASE_URL  public URL of the updates folder (required; environment or .env)
    SEAGLASS_UPDATER_PUBKEY   updater public key (default: <private key>.pub)
    SEAGLASS_RELEASE_DEST     publishing destination: local folder or UNC, or scp target
                              "user@host:/path" or "ssh-alias:/path"
    SEAGLASS_MPV_PATH         mpv.exe to bundle (default: the one on the PATH), matching
                              mpv.lock; frozen copy set up by update-mpv-lock.ps1
    SEAGLASS_SIGNING_KEY      updater private key (default: ~/.tauri/seaglass.key)
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD
                              key password; otherwise the generic Credential Manager entry
                              "seaglass-signing-key", otherwise asked on the keyboard
    SEAGLASS_GITHUB_TOKEN     GitHub token for the release (see github-release.ps1)

.EXAMPLE
  pnpm release                      # builds into release/, publishes nothing
  pnpm release -Publish             # builds, publishes and tags
  pnpm release -PublishOnly         # publishes the last build (made on this commit)
  pnpm release -Notes "- Playback fix"
#>
param(
    [switch]$Publish,
    [string]$Notes,
    # To test a build with uncommitted changes (refused with -Publish).
    # The installer of such a build is not compressed (makensis ~1 s instead of ~50 s).
    [switch]$AllowDirty,
    # Publishes the last build instead of making a new one (implies -Publish).
    [switch]$PublishOnly,
    # The GitHub release is made by the caller (release-all.ps1).
    [switch]$SkipGitHubRelease
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Fail($message) { Write-Host "ERROR: $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }
. (Join-Path $PSScriptRoot 'build-stamp.ps1')
. (Join-Path $PSScriptRoot 'updater-manifest.ps1')
. (Join-Path $PSScriptRoot 'release-notes.ps1')
if ($PublishOnly) { $Publish = $true }

# --- 1. Checks --------------------------------------------------------------------------
$version = (Get-Content VERSION -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { Fail "invalid VERSION: '$version'" }
$tag = "v$version"

$dirty = git status --porcelain
if ($dirty -and ($Publish -or -not $AllowDirty)) {
    Fail "there are uncommitted changes (use -AllowDirty for a test build)"
}
# The tag may already have been set on this commit by the Android release (release-android.ps1).
$tagExists = [bool](git tag --list $tag)
if ($Publish) {
    if ($tagExists -and (git rev-parse "$tag^{commit}") -ne (git rev-parse HEAD)) {
        Fail "tag $tag exists on another commit: bump VERSION before publishing"
    }
    if (-not $env:SEAGLASS_RELEASE_DEST) { Fail "SEAGLASS_RELEASE_DEST is not set" }
}
if (-not (Test-Path .env)) { Fail ".env missing: the client ID is injected at build time" }

$keyPath = if ($env:SEAGLASS_SIGNING_KEY) { $env:SEAGLASS_SIGNING_KEY } else { Join-Path $HOME '.tauri\seaglass.key' }
if (-not (Test-Path $keyPath)) { Fail "signing key not found: $keyPath" }

# Same address compiled into the app (#452, read by build.rs) and written into the manifests;
# public key compiled in too (#489). No default value.
. (Join-Path $PSScriptRoot 'release-env.ps1')
$baseUrl = Use-UpdateBaseUrl
Use-UpdaterPubkey $keyPath

if (-not $PublishOnly) {
    # --- 2. Bundled mpv ---------------------------------------------------------------------
    # Only the mpv described by scripts/mpv.lock (version + SHA-256, in git) goes into the
    # installer: otherwise an mpv replaced or updated on this PC would be distributed signed.
    Step "mpv"
    $lock = @{}
    Get-Content (Join-Path $PSScriptRoot 'mpv.lock') -Encoding UTF8 | Where-Object { $_ -match '^\s*[^#\s]' } | ForEach-Object {
        $name, $value = $_ -split '=', 2
        $lock[$name.Trim()] = "$value".Trim()
    }
    if (-not $lock.sha256 -or -not $lock.version) { Fail "scripts/mpv.lock incomplete (version and sha256 expected)" }
    $mpvSource = if ($env:SEAGLASS_MPV_PATH) { $env:SEAGLASS_MPV_PATH } else { (Get-Command mpv.exe -ErrorAction SilentlyContinue).Source }
    if (-not $mpvSource -or -not (Test-Path $mpvSource)) { Fail "mpv.exe not found (PATH or SEAGLASS_MPV_PATH)" }
    # Hash checked before running anything from this binary.
    $mpvHash = (Get-FileHash $mpvSource -Algorithm SHA256).Hash
    if ($mpvHash -ne $lock.sha256.ToUpperInvariant()) {
        Fail ("mpv.exe does not match scripts/mpv.lock`n    file     : $mpvSource`n    SHA-256  : $mpvHash`n" +
            "    expected : $($lock.sha256)`n    mpv changed (update, replacement?): check this binary, then freeze it and`n" +
            "    update mpv.lock with 'powershell -File scripts/update-mpv-lock.ps1' (to commit)")
    }
    $mpvVersion = ((& $mpvSource --version | Select-Object -First 1) -split '\s+' | Select-Object -First 2) -join ' '
    if ($mpvVersion -ne $lock.version) { Fail "unexpected mpv version: '$mpvVersion' (mpv.lock: '$($lock.version)')" }
    $sidecar = 'src-tauri\binaries\mpv-x86_64-pc-windows-msvc.exe'
    New-Item -ItemType Directory -Force (Split-Path $sidecar) | Out-Null
    Copy-Item $mpvSource $sidecar -Force
    if ((Get-FileHash $sidecar -Algorithm SHA256).Hash -ne $mpvHash) { Fail "corrupted mpv copy: $sidecar" }
    Write-Host "    $mpvVersion, SHA-256 matches mpv.lock"

    # --- 3. Signed build --------------------------------------------------------------------
    Step "signing key"
    . (Join-Path $PSScriptRoot 'signing-key.ps1')
    $keyPassword = Get-SigningKeyPassword $keyPath

    Step "build of Seaglass $version"
    $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
    # Empty = variable missing under PowerShell 5.1: Tauri then assumes an empty password.
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $keyPassword
    try {
        $updaterConfig = New-UpdaterConfig
        $configs = @('--config', (Join-Path $root 'src-tauri\tauri.release.conf.json'), '--config', $updaterConfig)
        if ($dirty) {
            # Test build, never publishable (-PublishOnly refuses a modified tree): the LZMA
            # compression of mpv (~50 s, a single core) only matters for downloads.
            Write-Host "    test build: uncompressed installer (~140 MB)" -ForegroundColor Yellow
            $configs += @('--config', (Join-Path $PSScriptRoot 'tauri.test.conf.json'))
        }
        pnpm tauri build @configs
        $buildExit = $LASTEXITCODE
    } finally {
        Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
        $keyPassword = $null
        if ($updaterConfig) { Remove-Item $updaterConfig -ErrorAction SilentlyContinue }
    }
    if ($buildExit -ne 0) { Fail "build failed" }

    $bundle = 'src-tauri\target\release\bundle\nsis'
    $setupName = "Seaglass_${version}_x64-setup.exe"
    $setup = Join-Path $bundle $setupName
    if (-not (Test-Path $setup) -or -not (Test-Path "$setup.sig")) { Fail "installer or signature missing in $bundle" }

    # --- 4. Files to publish ----------------------------------------------------------------
    Step "manifest"
    $out = Join-Path $root "release\$version"
    New-Item -ItemType Directory -Force $out | Out-Null
    Copy-Item $setup, "$setup.sig" $out -Force

    # From CHANGELOG.md (#49): latest.json is shared with Linux, windows.json feeds the website.
    if (-not $Notes) { $Notes = Get-ReleaseNotes $version 'desktop' }
    # Stamp before the manifest: it decides which platforms are announced (Linux if it was
    # built on this commit, #314).
    Write-BuildStamp 'windows' $version
    $announced = Write-UpdaterManifest $version $Notes $baseUrl
    $manifestPath = Join-Path $root 'release\latest.json'

    $item = Get-Item $setup
    $size = [math]::Round($item.Length / 1MB, 1)

    # Manifest of the website's download page (#517), same format as android.json: the page
    # is static (/download/ page of site/) and reads it from the updates folder.
    $download = [ordered]@{
        version  = $version
        url      = "$baseUrl/$version/$setupName"
        sha256   = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLowerInvariant()
        size     = $item.Length
        notes    = $Notes
        pub_date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    }
    $downloadPath = Join-Path $root 'release\windows.json'
    [System.IO.File]::WriteAllText($downloadPath, ($download | ConvertTo-Json -Depth 3), [System.Text.UTF8Encoding]::new($false))

    Write-Host "    installer: release\$version\$setupName ($size MB)"
    Write-Host "    manifest : release\latest.json -> $baseUrl/latest.json ($($announced -join ', '))"
    Write-Host "    page     : release\windows.json -> $baseUrl/windows.json"
} else {
    $setupName = "Seaglass_${version}_x64-setup.exe"
    $out = Join-Path $root "release\$version"
    $manifestPath = Join-Path $root 'release\latest.json'
    $downloadPath = Join-Path $root 'release\windows.json'
    Assert-BuildStamp 'windows' $version @((Join-Path $out $setupName), (Join-Path $out "$setupName.sig"), $manifestPath, $downloadPath)
    if ((Get-Content $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json).version -ne $version) {
        Fail "release\latest.json does not describe $version"
    }
    Write-Host "    reusing the $version build of this commit"
}

# --- 5. Publishing ----------------------------------------------------------------------
if (-not $Publish) {
    Write-Host "Build done, nothing was published (add -Publish)." -ForegroundColor Green
    exit 0
}

Step "publishing to $env:SEAGLASS_RELEASE_DEST"
$dest = $env:SEAGLASS_RELEASE_DEST
# latest.json may also announce the AppImage (#314): it must already be online.
Assert-ManifestOnline $manifestPath $dest @('windows-x86_64')
# The manifest goes last: the updater only sees the version once the installer is in place.
# scp target: "host:..." or "user@host:..." (at least 2 characters before ":", so as not to
# mistake it for a drive letter such as "D:").
if ($dest -match '^[^\\/:]{2,}:') {
    $sshHost, $remotePath = $dest -split ':', 2
    # Only the Windows files: release/<version>/ may also hold the APK.
    ssh -n $sshHost "mkdir -p '$remotePath/$version'"
    if ($LASTEXITCODE -ne 0) { Fail "could not create the version folder" }
    scp (Join-Path $out $setupName) (Join-Path $out "$setupName.sig") "$dest/$version/"
    if ($LASTEXITCODE -ne 0) { Fail "scp failed (installer)" }
    # scp carries over Windows permissions that are too wide (folder writable by all): reset
    # them to 755 / 644 before making the version visible.
    ssh -n $sshHost "chmod 755 '$remotePath/$version' && chmod 644 '$remotePath/$version'/*"
    if ($LASTEXITCODE -ne 0) { Fail "could not set the permissions on the server" }
    # The download page's manifest (#517), then the updater's last.
    scp $downloadPath "$dest/windows.json"
    if ($LASTEXITCODE -ne 0) { Fail "scp failed (windows.json)" }
    scp $manifestPath "$dest/latest.json"
    if ($LASTEXITCODE -ne 0) { Fail "scp failed (manifest)" }
    ssh -n $sshHost "chmod 644 '$remotePath/windows.json' '$remotePath/latest.json'"
} else {
    New-Item -ItemType Directory -Force $dest | Out-Null
    New-Item -ItemType Directory -Force (Join-Path $dest $version) | Out-Null
    Copy-Item (Join-Path $out $setupName), (Join-Path $out "$setupName.sig") (Join-Path $dest $version) -Force
    Copy-Item $downloadPath (Join-Path $dest 'windows.json') -Force
    Copy-Item $manifestPath (Join-Path $dest 'latest.json') -Force
}

if ($tagExists) {
    Write-Host "    tag $tag already set on HEAD (Android release)"
} else {
    Step "tag $tag"
    git tag -a $tag -m "Seaglass $version"
    git push origin $tag
    if ($LASTEXITCODE -ne 0) { Fail "tag push failed" }
}
Write-Host "Seaglass $version published." -ForegroundColor Green
if ($SkipGitHubRelease) { exit 0 }

# The update is already online: a failure here is only a warning (#108).
try {
    & (Join-Path $PSScriptRoot 'github-release.ps1') -Version $version
    $githubOk = $LASTEXITCODE -eq 0
} catch {
    Write-Host "ERROR: $_" -ForegroundColor Red
    $githubOk = $false
}
if (-not $githubOk) {
    Write-Host ("WARNING: GitHub release not created, the update stays published.`n" +
        "    To redo it alone: powershell -File scripts/github-release.ps1") -ForegroundColor Yellow
}
