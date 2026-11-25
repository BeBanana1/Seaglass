<#
.SYNOPSIS
  Builds and (on request) publishes the Seaglass Linux AppImage (#313).

.DESCRIPTION
  1. Checks the git tree, `.env`, the version (`VERSION`) and the WSL build distribution.
  2. Builds the frontend under Windows (`pnpm build`: Node 20+ is not in Ubuntu 22.04),
     then the AppImage in the WSL distribution (`linux-build.sh`). The build fails if the
     AppImage requires a newer glibc than the build distribution's.
  3. Puts the files to publish in `release/`:
       release/linux.json                          manifest read by the download page
       release/<version>/Seaglass_<version>_amd64.AppImage
  4. With -Publish: copies the AppImage then the manifest to SEAGLASS_RELEASE_DEST, sets the
     tag `v<version>` if it does not exist (otherwise it must point to HEAD), then attaches
     the AppImage to the tag's GitHub release (`github-release.ps1`, unless -SkipGitHubRelease).

  -PublishOnly skips steps 2 and 3 and publishes the last build, if it was made on this
  commit with a clean tree (`release-all.ps1` builds every platform before publishing).

  The AppImage is signed for the updater (#314) with the Windows installer's key (same
  password, `signing-key.ps1`), passed to WSL through the environment. The build rewrites
  `release/latest.json` (`updater-manifest.ps1`): it announces the AppImage, and the Windows
  installer if it was built on this commit. This manifest is only uploaded by the Windows
  publication, last (`release-all.ps1` publishes Linux before): published alone, the
  AppImage can be downloaded but is not offered as an update yet.

  The build distribution is set up once with `scripts/linux-build-setup.sh` (see
  docs/linux.md, "Release").

  Environment variables:
    SEAGLASS_LINUX_DISTRO      WSL build distribution (default: Ubuntu-22.04)
    SEAGLASS_LINUX_GLIBC_MAX   newest glibc allowed (default: 2.35, Ubuntu 22.04's)
    SEAGLASS_UPDATE_BASE_URL   public URL of the updates folder (required; environment or .env)
    SEAGLASS_UPDATER_PUBKEY    updater public key (default: <private key>.pub)
    SEAGLASS_RELEASE_DEST      publishing destination (see release.ps1)
    SEAGLASS_SIGNING_KEY       updater private key (default: ~/.tauri/seaglass.key)
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD
                               key password (see release.ps1)

.EXAMPLE
  powershell -File scripts/release-linux.ps1               # builds into release/
  powershell -File scripts/release-linux.ps1 -Publish      # builds, publishes and tags
  powershell -File scripts/release-linux.ps1 -PublishOnly  # publishes the last build
#>
param(
    [switch]$Publish,
    [string]$Notes,
    # To test a build with uncommitted changes (refused with -Publish).
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
$tagExists = [bool](git tag --list $tag)
if ($Publish) {
    if (-not $env:SEAGLASS_RELEASE_DEST) { Fail "SEAGLASS_RELEASE_DEST is not set" }
    if ($tagExists -and (git rev-parse "$tag^{commit}") -ne (git rev-parse HEAD)) {
        Fail "tag $tag exists on another commit: bump VERSION before publishing"
    }
}

# Same address compiled into the app (#452, read by build.rs, passed to WSL) and written into
# the manifest. No default value (#489).
. (Join-Path $PSScriptRoot 'release-env.ps1')
$baseUrl = Use-UpdateBaseUrl
$imageName = "Seaglass_${version}_amd64.AppImage"
$out = Join-Path $root "release\$version"
$imageOut = Join-Path $out $imageName
$sigOut = "$imageOut.sig"
$manifestPath = Join-Path $root 'release\linux.json'

if (-not $PublishOnly) {
    if (-not (Test-Path .env)) { Fail ".env missing: the client ID is injected at build time" }
    $distro = if ($env:SEAGLASS_LINUX_DISTRO) { $env:SEAGLASS_LINUX_DISTRO } else { 'Ubuntu-22.04' }
    $glibcMax = if ($env:SEAGLASS_LINUX_GLIBC_MAX) { $env:SEAGLASS_LINUX_GLIBC_MAX } else { '2.35' }
    if ($glibcMax -notmatch '^\d+\.\d+$') { Fail "invalid SEAGLASS_LINUX_GLIBC_MAX: '$glibcMax'" }
    $keyPath = if ($env:SEAGLASS_SIGNING_KEY) { $env:SEAGLASS_SIGNING_KEY } else { Join-Path $HOME '.tauri\seaglass.key' }
    if (-not (Test-Path $keyPath)) { Fail "signing key not found: $keyPath" }
    Use-UpdaterPubkey $keyPath

    # `wsl -l -q` writes UTF-16: the null characters are removed before comparing.
    $distros = @(wsl.exe -l -q | ForEach-Object { ($_ -replace "`0", '').Trim() } | Where-Object { $_ })
    if ($distros -notcontains $distro) {
        Fail ("WSL distribution '$distro' missing. Create it, then set it up:`n" +
            "    wsl --install -d $distro`n" +
            "    wsl -d $distro -u root bash scripts/linux-build-setup.sh --system`n" +
            "    wsl -d $distro bash scripts/linux-build-setup.sh")
    }

    Step "signing key"
    . (Join-Path $PSScriptRoot 'signing-key.ps1')
    $keyPassword = Get-SigningKeyPassword $keyPath

    # --- 2. Build ---------------------------------------------------------------------------
    Step "frontend"
    pnpm build
    if ($LASTEXITCODE -ne 0) { Fail "frontend build failed" }

    Step "Seaglass $version AppImage ($distro, glibc <= $glibcMax)"
    $toWsl = { param($path) (wsl.exe -d $distro wslpath -a ($path -replace '\\', '/')).Trim() }
    New-Item -ItemType Directory -Force $out | Out-Null
    Remove-Item $imageOut, $sigOut -ErrorAction SilentlyContinue
    $wslRoot = & $toWsl $root
    $wslOut = & $toWsl $out
    # Only the repository's files go into WSL (#418): reading /mnt/g there is costly per
    # file, and the tree holds tens of thousands of files the build does not need
    # (worktrees, `relay/target`, Android builds). Tracked files and untracked files that
    # are not ignored (test build with -AllowDirty), except deleted ones, plus `.env` and
    # `dist/`.
    $buildDir = Join-Path $root 'release\.build'
    New-Item -ItemType Directory -Force $buildDir | Out-Null
    $fileList = Join-Path $buildDir 'linux-files.txt'
    $files = @(git -c core.quotepath=off ls-files --cached --others --exclude-standard | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
    if ($LASTEXITCODE -ne 0 -or -not $files) { Fail "could not list the repository's files" }
    $files += '.env'
    $files += @(Get-ChildItem dist -Recurse -File | ForEach-Object { (Resolve-Path -Relative $_.FullName).Substring(2) -replace '\\', '/' })
    [System.IO.File]::WriteAllText($fileList, (($files | Sort-Object -Unique) -join "`n") + "`n", [System.Text.UTF8Encoding]::new($false))
    $wslFileList = & $toWsl $fileList
    # Key and password passed to WSL through the environment (WSLENV), never through a file;
    # removed as soon as the build ends.
    $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $keyPath -Raw).Trim()
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $keyPassword
    $savedWslEnv = $env:WSLENV
    $env:WSLENV = (@($env:WSLENV, 'TAURI_SIGNING_PRIVATE_KEY', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'SEAGLASS_UPDATE_BASE_URL', 'SEAGLASS_UPDATER_PUBKEY') | Where-Object { $_ }) -join ':'
    try {
        # Output shown as is (not captured); the AppImage's name is known in advance.
        wsl.exe -d $distro --cd $wslRoot bash scripts/linux-build.sh $wslRoot $wslOut $glibcMax $wslFileList
        $buildExit = $LASTEXITCODE
    } finally {
        Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
        if ($savedWslEnv) { $env:WSLENV = $savedWslEnv } else { Remove-Item Env:WSLENV -ErrorAction SilentlyContinue }
        $keyPassword = $null
    }
    if ($buildExit -ne 0) { Fail "Linux build failed" }
    if (-not (Test-Path $imageOut) -or -not (Test-Path $sigOut)) { Fail "AppImage or signature missing in release\$version" }

    # --- 3. Files to publish ----------------------------------------------------------------
    Step "manifest"
    # From CHANGELOG.md (#49): linux.json gets Linux's entries, latest.json (shared with
    # Windows) the desktop ones.
    $desktopNotes = if ($Notes) { $Notes } else { Get-ReleaseNotes $version 'desktop' }
    if (-not $Notes) { $Notes = Get-ReleaseNotes $version 'linux' }
    $item = Get-Item $imageOut
    $manifest = [ordered]@{
        version  = $version
        url      = "$baseUrl/$version/$imageName"
        sha256   = (Get-FileHash $imageOut -Algorithm SHA256).Hash.ToLowerInvariant()
        size     = $item.Length
        glibc    = $glibcMax
        notes    = $Notes
        pub_date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    }
    [System.IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 3), [System.Text.UTF8Encoding]::new($false))
    $size = [math]::Round($item.Length / 1MB, 1)
    Write-BuildStamp 'linux' $version
    $announced = Write-UpdaterManifest $version $desktopNotes $baseUrl
    Write-Host "    AppImage: release\$version\$imageName ($size MB)"
    Write-Host "    manifest: release\linux.json -> $baseUrl/linux.json"
    Write-Host "    updater : release\latest.json ($($announced -join ', ')), uploaded by the Windows publication"
} else {
    Assert-BuildStamp 'linux' $version @($imageOut, $sigOut, $manifestPath)
    $manifest = Get-Content $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($manifest.version -ne $version) { Fail "release\linux.json does not describe $version" }
    if ((Get-FileHash $imageOut -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.sha256) {
        Fail "the AppImage no longer matches release\linux.json"
    }
    Write-Host "    reusing the $version build of this commit"
}

# --- 4. Publishing ----------------------------------------------------------------------
if (-not $Publish) {
    Write-Host "Build done, nothing was published (add -Publish)." -ForegroundColor Green
    exit 0
}

Step "publishing to $env:SEAGLASS_RELEASE_DEST"
$dest = $env:SEAGLASS_RELEASE_DEST
# The manifest goes last: the page only offers the version once the AppImage is in place.
if ($dest -match '^[^\\/:]{2,}:') {
    $sshHost, $remotePath = $dest -split ':', 2
    ssh -n $sshHost "mkdir -p '$remotePath/$version' && chmod 755 '$remotePath/$version'"
    if ($LASTEXITCODE -ne 0) { Fail "could not create the version folder" }
    scp $imageOut $sigOut "$dest/$version/"
    if ($LASTEXITCODE -ne 0) { Fail "scp failed (AppImage)" }
    scp $manifestPath "$dest/linux.json"
    if ($LASTEXITCODE -ne 0) { Fail "scp failed (manifest)" }
    ssh -n $sshHost "chmod 644 '$remotePath/$version/$imageName' '$remotePath/$version/$imageName.sig' '$remotePath/linux.json'"
    if ($LASTEXITCODE -ne 0) { Fail "could not set the permissions on the server" }
} else {
    New-Item -ItemType Directory -Force (Join-Path $dest $version) | Out-Null
    Copy-Item $imageOut, $sigOut (Join-Path $dest $version) -Force
    Copy-Item $manifestPath (Join-Path $dest 'linux.json') -Force
}

if ($tagExists) {
    Write-Host "    tag $tag already set on HEAD"
} else {
    Step "tag $tag"
    git tag -a $tag -m "Seaglass $version"
    git push origin $tag
    if ($LASTEXITCODE -ne 0) { Fail "tag push failed" }
}
Write-Host "Seaglass $version AppImage published." -ForegroundColor Green
Write-Host "    updates: offered once latest.json is uploaded by the Windows publication (release.ps1)"
if ($SkipGitHubRelease) { exit 0 }

# The AppImage is already online: a failure here is only a warning (#108).
try {
    & (Join-Path $PSScriptRoot 'github-release.ps1') -Version $version
    $githubOk = $LASTEXITCODE -eq 0
} catch {
    Write-Host "ERROR: $_" -ForegroundColor Red
    $githubOk = $false
}
if (-not $githubOk) {
    Write-Host ("WARNING: AppImage not attached to the GitHub release, it stays published.`n" +
        "    To redo it alone: powershell -File scripts/github-release.ps1") -ForegroundColor Yellow
}
