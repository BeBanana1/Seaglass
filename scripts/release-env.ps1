# Settings of the release scripts (#489), with no default value specific to the author.
# Load with ". (Join-Path $PSScriptRoot 'release-env.ps1')"; assumes `Fail` is defined.

# Value of a setting: environment variable, otherwise `NAME=value` line of the root `.env`,
# otherwise $null. Same order as `src-tauri/build.rs`.
function Get-ReleaseSetting([string]$name) {
    $value = [Environment]::GetEnvironmentVariable($name)
    if ($value -and $value.Trim()) { return $value.Trim() }
    $envFile = Join-Path (Split-Path -Parent $PSScriptRoot) '.env'
    if (-not (Test-Path $envFile)) { return $null }
    foreach ($line in Get-Content $envFile -Encoding UTF8) {
        $line = $line.Trim()
        if ($line.StartsWith('#')) { continue }
        $key, $rest = $line -split '=', 2
        if ($key.Trim() -eq $name -and $rest) {
            $rest = $rest.Trim().Trim('"')
            if ($rest) { return $rest }
        }
    }
    return $null
}

# Public folder of the updates, without a trailing `/`. Required: it is compiled into the app
# (build.rs) and written into the manifests. Set in the environment for the build.
function Use-UpdateBaseUrl {
    $url = Get-ReleaseSetting 'SEAGLASS_UPDATE_BASE_URL'
    if (-not $url) {
        Fail ("SEAGLASS_UPDATE_BASE_URL is not set (environment or .env): public address`n" +
            "    of the updates folder, for example https://updates.example.org/seaglass")
    }
    $url = $url.TrimEnd('/')
    $env:SEAGLASS_UPDATE_BASE_URL = $url
    return $url
}

# Public key of the desktop updater, compiled into the app (build.rs): variable
# SEAGLASS_UPDATER_PUBKEY, otherwise the `<private key>.pub` file of `tauri signer generate`.
# Set in the environment for the build.
function Use-UpdaterPubkey([string]$keyPath) {
    $pubkey = Get-ReleaseSetting 'SEAGLASS_UPDATER_PUBKEY'
    if (-not $pubkey) {
        $pubPath = "$keyPath.pub"
        if (-not (Test-Path $pubPath)) {
            Fail "updater public key not found: $pubPath (or variable SEAGLASS_UPDATER_PUBKEY)"
        }
        $pubkey = (Get-Content $pubPath -Raw).Trim()
    }
    $env:SEAGLASS_UPDATER_PUBKEY = $pubkey
}

# `--config` file of the Tauri CLI with the public key set by Use-UpdaterPubkey: the CLI
# checks the signature of the updater artifacts with `plugins.updater.pubkey`, which is empty
# in `tauri.conf.json`. Temporary file (UTF-8 without BOM), to delete after the build.
function New-UpdaterConfig {
    $path = [IO.Path]::GetTempFileName()
    $json = '{"plugins":{"updater":{"pubkey":"' + $env:SEAGLASS_UPDATER_PUBKEY + '"}}}'
    [IO.File]::WriteAllText($path, $json, (New-Object Text.UTF8Encoding $false))
    return $path
}
