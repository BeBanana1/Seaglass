<#
.SYNOPSIS
  Builds, signs and (on request) publishes the Seaglass Android APK (#24).

.DESCRIPTION
  1. Checks the git tree, `.env`, the Android release key and the version (`VERSION`).
  2. Checks the key password (keytool), then builds the signed arm64 release APK
     (`pnpm tauri android build --apk --target aarch64`).
  3. Checks the signature (apksigner: at least scheme v2, not the debug key).
  4. Puts the files to publish in `release/`:
       release/android.json                       manifest read by the app (apk_update.rs)
       release/<version>/Seaglass_<version>_arm64.apk
  5. With -Publish: copies the APK then the manifest to SEAGLASS_RELEASE_DEST, sets the tag
     `v<version>` if it does not exist (same tag as the Windows installer: it must then
     point to HEAD), then attaches the APK to the tag's GitHub release (`github-release.ps1`,
     unless -SkipGitHubRelease).

  -PublishOnly skips steps 2 to 4 and publishes the last build, if it was made on this
  commit with a clean tree (`release-all.ps1` builds Windows and Android before publishing).

  The release key is created once (see docs/android.md, "Distribution" bullet) and backed
  up like the minisign key: Android refuses any update signed by another key than the
  installed app's.

  Environment variables:
    SEAGLASS_ANDROID_KEYSTORE      PKCS12 keystore (default: ~/.android/seaglass-release.jks)
    SEAGLASS_ANDROID_KEY_ALIAS     key alias (default: seaglass)
    SEAGLASS_ANDROID_KEY_PASSWORD  password; otherwise the generic Credential Manager
                                   entry "seaglass-android-key", otherwise asked on the
                                   keyboard
    SEAGLASS_UPDATE_BASE_URL       public URL of the updates folder
                                   (required; environment or .env; compiled into the app,
                                   which only accepts this address)
    SEAGLASS_RELEASE_DEST          publishing destination (see release.ps1)
    ANDROID_HOME, NDK_HOME         default: %LOCALAPPDATA%\Android\Sdk and its latest NDK

.EXAMPLE
  powershell -File scripts/release-android.ps1             # builds into release/
  powershell -File scripts/release-android.ps1 -Publish    # builds, publishes and tags
  powershell -File scripts/release-android.ps1 -PublishOnly  # publishes the last build
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
if (-not (Test-Path .env)) { Fail ".env missing: the client ID is injected at build time" }

$keystore = if ($env:SEAGLASS_ANDROID_KEYSTORE) { $env:SEAGLASS_ANDROID_KEYSTORE } else { Join-Path $HOME '.android\seaglass-release.jks' }
if (-not (Test-Path $keystore)) { Fail "Android release key not found: $keystore (to create it: docs/android.md, 'Distribution' bullet)" }
$keystore = (Resolve-Path $keystore).Path
$alias = if ($env:SEAGLASS_ANDROID_KEY_ALIAS) { $env:SEAGLASS_ANDROID_KEY_ALIAS } else { 'seaglass' }

# Same address compiled into the app (#452, read by build.rs) and written into the manifest.
# No default value (#489).
. (Join-Path $PSScriptRoot 'release-env.ps1')
$baseUrl = Use-UpdateBaseUrl

# Android tools.
$sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
if (-not (Test-Path $sdk)) { Fail "Android SDK not found: $sdk" }
$env:ANDROID_HOME = $sdk
if (-not $env:NDK_HOME) {
    $ndk = Get-ChildItem (Join-Path $sdk 'ndk') -Directory -ErrorAction SilentlyContinue |
        Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    if (-not $ndk) { Fail "NDK not found in $sdk\ndk" }
    $env:NDK_HOME = $ndk.FullName
}
$buildTools = Get-ChildItem (Join-Path $sdk 'build-tools') -Directory -ErrorAction SilentlyContinue |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (-not $buildTools) { Fail "build-tools not found in $sdk\build-tools" }
$apksigner = Join-Path $buildTools.FullName 'apksigner.bat'
$keytool = if ($env:JAVA_HOME -and (Test-Path (Join-Path $env:JAVA_HOME 'bin\keytool.exe'))) {
    Join-Path $env:JAVA_HOME 'bin\keytool.exe'
} else { (Get-Command keytool -ErrorAction SilentlyContinue).Source }
if (-not $keytool) { Fail "keytool not found (JAVA_HOME or PATH)" }

if (-not $PublishOnly) {
    # --- 2. Password, then build ------------------------------------------------------------
    # As for the minisign key (#87): session variable, otherwise Credential Manager,
    # otherwise typed. It only goes through the build's environment.
    Step "Android release key"
    . (Join-Path $PSScriptRoot 'credentials.ps1')
    $credentialTarget = 'seaglass-android-key'
    if ($env:SEAGLASS_ANDROID_KEY_PASSWORD) {
        $keyPassword = $env:SEAGLASS_ANDROID_KEY_PASSWORD
        $origin = 'SEAGLASS_ANDROID_KEY_PASSWORD'
    } else {
        $keyPassword = Read-StoredSecret $credentialTarget
        $origin = "Credential Manager ($credentialTarget)"
    }
    if (-not $keyPassword) {
        $origin = 'typed'
        try { $secure = Read-Host "    Android key password" -AsSecureString }
        catch { Fail "key password missing (non-interactive session): set SEAGLASS_ANDROID_KEY_PASSWORD" }
        $keyPassword = [System.Net.NetworkCredential]::new('', $secure).Password
    }
    if (-not $keyPassword) { Fail "empty key password ($origin)" }

    $env:SEAGLASS_ANDROID_KEYSTORE = $keystore
    $env:SEAGLASS_ANDROID_KEY_ALIAS = $alias
    $env:SEAGLASS_ANDROID_KEY_PASSWORD = $keyPassword
    try {
        # -storepass:env: the password does not appear on the command line.
        & $keytool -list -keystore $keystore -storetype PKCS12 -alias $alias -storepass:env SEAGLASS_ANDROID_KEY_PASSWORD *> $null
        if ($LASTEXITCODE -ne 0) { Fail "wrong key password or alias ($origin, alias $alias)" }
        Write-Host "    password checked ($origin)"

        Step "build of the $version APK"
        pnpm tauri android build --apk --target aarch64
        $buildExit = $LASTEXITCODE
    } finally {
        Remove-Item Env:SEAGLASS_ANDROID_KEY_PASSWORD -ErrorAction SilentlyContinue
        $keyPassword = $null
    }
    if ($buildExit -ne 0) { Fail "build failed" }

    $apk = 'src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release.apk'
    if (-not (Test-Path $apk)) { Fail "signed APK missing: $apk (key not picked up by Gradle?)" }

    # --- 3. Signature -----------------------------------------------------------------------
    Step "signature"
    # apksigner writes a JDK warning to stderr ("A restricted method in java.lang.System has
    # been called"): under PowerShell 5.1, with $ErrorActionPreference = 'Stop', `2>&1` turns
    # it into a terminating error (#443). Only the exit code matters here; the lines are
    # turned back into text for the filters.
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $certs = & $apksigner verify --min-sdk-version 24 --print-certs --verbose $apk 2>&1 | ForEach-Object { "$_" }
        $verifyExit = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($verifyExit -ne 0) { Fail "invalid APK signature:`n$($certs -join "`n")" }
    if (-not ($certs -match 'Verified using v2 scheme \(APK Signature Scheme v2\): true')) { Fail "APK not signed with scheme v2" }
    if ($certs -match 'CN=Android Debug') { Fail "APK signed with the debug key" }
    $certSha = ($certs | Select-String 'Signer #1 certificate SHA-256 digest: (.+)$').Matches.Groups[1].Value
    Write-Host "    certificate SHA-256: $certSha"

    # --- 4. Files to publish ----------------------------------------------------------------
    Step "manifest"
    $out = Join-Path $root "release\$version"
    New-Item -ItemType Directory -Force $out | Out-Null
    $apkName = "Seaglass_${version}_arm64.apk"
    $apkOut = Join-Path $out $apkName
    Copy-Item $apk $apkOut -Force

    # From CHANGELOG.md (#49).
    if (-not $Notes) { $Notes = Get-ReleaseNotes $version 'android' }
    $item = Get-Item $apkOut
    $manifest = [ordered]@{
        version  = $version
        url      = "$baseUrl/$version/$apkName"
        sha256   = (Get-FileHash $apkOut -Algorithm SHA256).Hash.ToLowerInvariant()
        size     = $item.Length
        notes    = $Notes
        pub_date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    }
    $manifestPath = Join-Path $root 'release\android.json'
    [System.IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 3), [System.Text.UTF8Encoding]::new($false))
    $size = [math]::Round($item.Length / 1MB, 1)
    Write-Host "    APK     : release\$version\$apkName ($size MB)"
    Write-Host "    manifest: release\android.json -> $baseUrl/android.json"
    Write-BuildStamp 'android' $version
} else {
    $apkName = "Seaglass_${version}_arm64.apk"
    $apkOut = Join-Path $root "release\$version\$apkName"
    $manifestPath = Join-Path $root 'release\android.json'
    Assert-BuildStamp 'android' $version @($apkOut, $manifestPath)
    $manifest = Get-Content $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($manifest.version -ne $version) { Fail "release\android.json does not describe $version" }
    if ((Get-FileHash $apkOut -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.sha256) {
        Fail "the APK no longer matches release\android.json"
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
# The manifest goes last: the app only sees the version once the APK is in place.
if ($dest -match '^[^\\/:]{2,}:') {
    $sshHost, $remotePath = $dest -split ':', 2
    ssh -n $sshHost "mkdir -p '$remotePath/$version' && chmod 755 '$remotePath/$version'"
    if ($LASTEXITCODE -ne 0) { Fail "could not create the version folder" }
    scp $apkOut "$dest/$version/$apkName"
    if ($LASTEXITCODE -ne 0) { Fail "scp failed (APK)" }
    scp $manifestPath "$dest/android.json"
    if ($LASTEXITCODE -ne 0) { Fail "scp failed (manifest)" }
    ssh -n $sshHost "chmod 644 '$remotePath/$version/$apkName' '$remotePath/android.json'"
    if ($LASTEXITCODE -ne 0) { Fail "could not set the permissions on the server" }
} else {
    New-Item -ItemType Directory -Force (Join-Path $dest $version) | Out-Null
    Copy-Item $apkOut (Join-Path $dest "$version\$apkName") -Force
    Copy-Item $manifestPath (Join-Path $dest 'android.json') -Force
}

if ($tagExists) {
    Write-Host "    tag $tag already set on HEAD (Windows release)"
} else {
    Step "tag $tag"
    git tag -a $tag -m "Seaglass $version"
    git push origin $tag
    if ($LASTEXITCODE -ne 0) { Fail "tag push failed" }
}
Write-Host "Seaglass $version APK published." -ForegroundColor Green
if ($SkipGitHubRelease) { exit 0 }

# The APK is already online: a failure here is only a warning (#108).
try {
    & (Join-Path $PSScriptRoot 'github-release.ps1') -Version $version
    $githubOk = $LASTEXITCODE -eq 0
} catch {
    Write-Host "ERROR: $_" -ForegroundColor Red
    $githubOk = $false
}
if (-not $githubOk) {
    Write-Host ("WARNING: APK not attached to the GitHub release, it stays published.`n" +
        "    To redo it alone: powershell -File scripts/github-release.ps1") -ForegroundColor Yellow
}
