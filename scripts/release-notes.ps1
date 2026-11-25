# Release notes from CHANGELOG.md (#49), for the update manifests (latest.json, windows.json,
# android.json, linux.json: the app's update dialog and the website's download page) and the
# GitHub release. Before, they were the commit subjects since the previous tag (ci:, chore:,
# docs: and all), which users read in the update dialog.
# Load with ". (Join-Path $PSScriptRoot 'release-notes.ps1')".
#
# A release covers every version since the previous v* tag, so the notes hold the entries of
# all of them, newest first. Entries are filtered like the app's version log
# (src-tauri/src/commands/changelog.rs): `chore:` dropped, a platform prefix only kept where it
# applies, and removed when it is the platform's own.
#   windows, linux, android: the manifests of one platform
#   desktop: latest.json, shared by Windows and Linux (keeps "Windows:" and "Linux:" visible)
#   all: the GitHub release, which holds every platform (keeps every prefix)

# Platforms that keep an entry with this prefix.
$ReleaseNotesPrefixes = [ordered]@{
    'Android:' = @('android')
    'Windows:' = @('windows', 'desktop')
    'Linux:'   = @('linux', 'desktop')
    'Desktop:' = @('windows', 'linux', 'desktop')
}

# The entry as shown on $platform, or $null when it is not for it.
function Select-ReleaseNotesEntry([string]$text, [string]$platform) {
    if ($text.StartsWith('chore:', [StringComparison]::Ordinal)) { return $null }
    if ($platform -eq 'all') { return $text }
    foreach ($prefix in $ReleaseNotesPrefixes.Keys) {
        if (-not $text.StartsWith($prefix, [StringComparison]::Ordinal)) { continue }
        if ($ReleaseNotesPrefixes[$prefix] -notcontains $platform) { return $null }
        if ($platform -eq 'desktop' -and $prefix -ne 'Desktop:') { return $text }
        # "Desktop: the bars…" → "The bars…"
        $rest = $text.Substring($prefix.Length).TrimStart()
        if (-not $rest) { return $null }
        return $rest.Substring(0, 1).ToUpperInvariant() + $rest.Substring(1)
    }
    $text
}

# Highest v* tag below $version, as a [version], or $null.
function Get-PreviousReleaseVersion([version]$version) {
    $previous = $null
    foreach ($tag in @(git tag --list 'v*')) {
        $parsed = $null
        if (-not [version]::TryParse($tag.Substring(1), [ref]$parsed)) { continue }
        if ($parsed -lt $version -and (-not $previous -or $parsed -gt $previous)) { $previous = $parsed }
    }
    $previous
}

# "- entry" lines of the versions after the previous tag up to $version, for $platform. Throws if
# CHANGELOG.md has no section for $version: a release without its log entry is a mistake.
function Get-ReleaseNotes([string]$version, [ValidateSet('windows', 'linux', 'android', 'desktop', 'all')][string]$platform) {
    $target = [version]$version
    $previous = Get-PreviousReleaseVersion $target
    $path = Join-Path (Split-Path -Parent $PSScriptRoot) 'CHANGELOG.md'
    # Read as UTF-8 whatever the console's code page: the log holds "›" and other non-ASCII text.
    $lines = [IO.File]::ReadAllText($path, [Text.Encoding]::UTF8) -split '\r?\n'

    $found = $false
    $inRange = $false
    $entries = New-Object System.Collections.Generic.List[string]
    foreach ($line in $lines) {
        if ($line.StartsWith('## ', [StringComparison]::Ordinal)) {
            $title = ($line.Substring(3) -split ' ', 2)[0]
            $section = $null
            $inRange = [version]::TryParse($title, [ref]$section) -and
                $section -le $target -and (-not $previous -or $section -gt $previous)
            if ($inRange -and $section -eq $target) { $found = $true }
            continue
        }
        if (-not $inRange -or -not $line.StartsWith('- ', [StringComparison]::Ordinal)) { continue }
        $kept = Select-ReleaseNotesEntry $line.Substring(2) $platform
        if ($kept) { $entries.Add("- $kept") }
    }
    if (-not $found) { throw "CHANGELOG.md has no '## $version' section: add the version's entry first" }
    $entries -join "`n"
}
