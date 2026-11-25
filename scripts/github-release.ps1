<#
.SYNOPSIS
  Creates (or completes) the GitHub release of a version already published (#108).

.DESCRIPTION
  Called by `release.ps1`, `release-android.ps1` or `release-linux.ps1` with -Publish, or by
  `release-all.ps1` once the tag is pushed, or alone if that step failed. Nothing is rebuilt:
  the attachments are exactly the files published on the update server, taken from
  `release/` (those that exist; at least the installer, the APK or the AppImage):
    release/<version>/Seaglass_<version>_x64-setup.exe
    release/<version>/Seaglass_<version>_x64-setup.exe.sig   (if it exists)
    release/latest.json                       (only if it describes this version)
    release/<version>/Seaglass_<version>_arm64.apk
    release/android.json                      (only if it describes this version)
    release/<version>/Seaglass_<version>_amd64.AppImage
    release/<version>/Seaglass_<version>_amd64.AppImage.sig  (if it exists)
    release/linux.json                        (only if it describes this version)

  The release uses the `v<version>` tag (which must already be on origin), the title
  "Seaglass <version>" and the entries of CHANGELOG.md since the previous tag, every platform
  (#49; without a log section for the version: the manifest's notes, then the git log). If it already exists, its notes stay and attachments with the same name are replaced.

  Goes through the GitHub CLI (`gh`), never through its default login: the token is given
  explicitly, so that a release never goes out under another account signed in on the PC.

  Settings:
    SEAGLASS_GITHUB_TOKEN  token of the account that owns the release (fine-grained,
                           "Contents: Read and write" on the repository; environment or
                           .env, never embedded in the app by build.rs); otherwise the
                           generic Credential Manager entry "seaglass-github-token"
                           (cmdkey /generic:seaglass-github-token /user:seaglass /pass)
    SEAGLASS_GITHUB_REPO   repository `owner/name` (required; environment or .env)
    SEAGLASS_GITHUB_USER   expected login of the token (optional; checked if set)

.EXAMPLE
  powershell -File scripts/github-release.ps1   # version from VERSION
  powershell -File scripts/github-release.ps1 -Version 0.4.7
#>
param([string]$Version)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
. (Join-Path $PSScriptRoot 'credentials.ps1')
. (Join-Path $PSScriptRoot 'release-notes.ps1')

function Fail($message) { Write-Host "ERROR: $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { Fail "GitHub CLI (gh) not found" }
if (-not $Version) { $Version = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim() }
if ($Version -notmatch '^\d+\.\d+\.\d+$') { Fail "invalid version: '$Version'" }
$tag = "v$Version"
# No default value (#489).
. (Join-Path $PSScriptRoot 'release-env.ps1')
$repo = Get-ReleaseSetting 'SEAGLASS_GITHUB_REPO'
if (-not $repo) { Fail "SEAGLASS_GITHUB_REPO is required (environment or .env)" }
$expectedUser = Get-ReleaseSetting 'SEAGLASS_GITHUB_USER'

Step "GitHub release $tag"

# --- Files ------------------------------------------------------------------------------
# A manifest is only attached to the version it describes (the last one built); its notes are
# the fallback when CHANGELOG.md has no section for the version.
function Read-Manifest([string]$name) {
    $path = Join-Path $root "release\$name"
    if (-not (Test-Path $path)) { return $null }
    $manifest = Get-Content $path -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($manifest.version -ne $Version) {
        Write-Host "    release\$name does not describe $Version`: not attached"
        return $null
    }
    @{ path = $path; notes = $manifest.notes }
}

$files = @()
$notes = $null
$setup = Join-Path $root "release\$Version\Seaglass_${Version}_x64-setup.exe"
if (Test-Path $setup) {
    $files += $setup
    # Versions before #108 did not keep the signature in release/.
    if (Test-Path "$setup.sig") { $files += "$setup.sig" }
    $desktop = Read-Manifest 'latest.json'
    if ($desktop) { $files += $desktop.path; $notes = $desktop.notes }
}
$apk = Join-Path $root "release\$Version\Seaglass_${Version}_arm64.apk"
if (Test-Path $apk) {
    $files += $apk
    $android = Read-Manifest 'android.json'
    if ($android) {
        $files += $android.path
        if (-not $notes) { $notes = $android.notes }
    }
}
$appImage = Join-Path $root "release\$Version\Seaglass_${Version}_amd64.AppImage"
if (Test-Path $appImage) {
    $files += $appImage
    # Updater signature, since #314.
    if (Test-Path "$appImage.sig") { $files += "$appImage.sig" }
    $linux = Read-Manifest 'linux.json'
    if ($linux) {
        $files += $linux.path
        if (-not $notes) { $notes = $linux.notes }
    }
}
if (-not $files) { Fail "no installer, APK or AppImage of $Version in release\" }

# The release holds every platform: all of the log's entries, prefixes kept (#49).
try {
    $fromLog = Get-ReleaseNotes $Version 'all'
    if ($fromLog) { $notes = $fromLog; Write-Host "    notes taken from CHANGELOG.md" }
} catch {
    Write-Host "    $($_.Exception.Message): manifest notes kept"
}
if (-not $notes) {
    Write-Host "    notes taken from git"
    $tags = @(git tag --list 'v*' --sort=v:refname)
    $index = [array]::IndexOf($tags, $tag)
    $range = if ($index -gt 0) { "$($tags[$index - 1])..$tag" } else { $tag }
    $notes = (git log $range --no-merges --pretty=format:'- %s' -n 30) -join "`n"
}

# Without the tag on origin, `gh release create` would make one on the default branch.
$remoteTag = git ls-remote --tags origin "refs/tags/$tag"
if ($LASTEXITCODE -ne 0) { Fail "cannot read the tags of origin" }
if (-not $remoteTag) { Fail "tag $tag is not on origin (push it first)" }

# --- Token ------------------------------------------------------------------------------
# Never shown nor written; only this process's `gh` calls see it.
$token = Get-ReleaseSetting 'SEAGLASS_GITHUB_TOKEN'
if (-not $token) { $token = Read-StoredSecret 'seaglass-github-token' }
if (-not $token) {
    Fail "GitHub token missing: set SEAGLASS_GITHUB_TOKEN (environment or .env) or the Credential Manager entry 'seaglass-github-token'"
}
$previousToken = $env:GH_TOKEN
$env:GH_TOKEN = $token
$token = $null

# Under Windows PowerShell 5.1, 'Stop' turns any stderr line of a native command into a
# terminating error ("release not found" is an expected answer): every `gh` call below
# checks $LASTEXITCODE instead.
$ErrorActionPreference = 'Continue'
try {
    $login = gh api user --jq .login 2>$null
    if ($LASTEXITCODE -ne 0 -or -not $login) { Fail "GitHub token refused" }
    if ($expectedUser -and $login -ne $expectedUser) {
        Fail "the token belongs to '$login', not to '$expectedUser'"
    }
    Write-Host "    as $login"

    gh release view $tag -R $repo --json id 2>$null | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "    existing release, attachments updated"
    } else {
        $notesFile = New-TemporaryFile
        try {
            [System.IO.File]::WriteAllText($notesFile.FullName, $notes, [System.Text.UTF8Encoding]::new($false))
            gh release create $tag -R $repo --verify-tag --title "Seaglass $Version" --notes-file $notesFile.FullName | Out-Null
            if ($LASTEXITCODE -ne 0) { Fail "release creation failed" }
        } finally { Remove-Item $notesFile.FullName -ErrorAction SilentlyContinue }
        Write-Host "    release created"
    }

    # --clobber replaces an attachment with the same name.
    gh release upload $tag -R $repo --clobber @files
    if ($LASTEXITCODE -ne 0) { Fail "attachment upload failed" }
    foreach ($file in $files) {
        $size = [math]::Round((Get-Item $file).Length / 1MB, 1)
        Write-Host "    $(Split-Path $file -Leaf) ($size MB)"
    }
} finally { $env:GH_TOKEN = $previousToken }

Write-Host "GitHub release: https://github.com/$repo/releases/tag/$tag" -ForegroundColor Green
exit 0
