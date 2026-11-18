# Réglages des scripts de release (#489), sans aucune valeur par défaut propre à l'auteur.
# À charger avec « . (Join-Path $PSScriptRoot 'release-env.ps1') » ; suppose `Fail` défini.

# Valeur d'un réglage : variable d'environnement, sinon ligne `NOM=valeur` du `.env` de la
# racine, sinon $null. Même ordre que `src-tauri/build.rs`.
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

# Dossier public des mises à jour, sans `/` final. Requis : il est compilé dans l'app
# (build.rs) et écrit dans les manifestes. Posé dans l'environnement pour la compilation.
function Use-UpdateBaseUrl {
    $url = Get-ReleaseSetting 'SEAGLASS_UPDATE_BASE_URL'
    if (-not $url) {
        Fail ("SEAGLASS_UPDATE_BASE_URL n'est pas défini (environnement ou .env) : adresse publique`n" +
            "    du dossier des mises à jour, par exemple https://updates.example.org/seaglass")
    }
    $url = $url.TrimEnd('/')
    $env:SEAGLASS_UPDATE_BASE_URL = $url
    return $url
}

# Clé publique de l'updater du bureau, compilée dans l'app (build.rs) : variable
# SEAGLASS_UPDATER_PUBKEY, sinon fichier `<clé privée>.pub` de `tauri signer generate`.
# Posée dans l'environnement pour la compilation.
function Use-UpdaterPubkey([string]$keyPath) {
    $pubkey = Get-ReleaseSetting 'SEAGLASS_UPDATER_PUBKEY'
    if (-not $pubkey) {
        $pubPath = "$keyPath.pub"
        if (-not (Test-Path $pubPath)) {
            Fail "clé publique de l'updater introuvable : $pubPath (ou variable SEAGLASS_UPDATER_PUBKEY)"
        }
        $pubkey = (Get-Content $pubPath -Raw).Trim()
    }
    $env:SEAGLASS_UPDATER_PUBKEY = $pubkey
}

# Fichier `--config` du CLI Tauri avec la clé publique posée par Use-UpdaterPubkey : le CLI
# vérifie la signature des artefacts de l'updater avec `plugins.updater.pubkey`, vide dans
# `tauri.conf.json`. Fichier temporaire (UTF-8 sans BOM), à supprimer après le build.
function New-UpdaterConfig {
    $path = [IO.Path]::GetTempFileName()
    $json = '{"plugins":{"updater":{"pubkey":"' + $env:SEAGLASS_UPDATER_PUBKEY + '"}}}'
    [IO.File]::WriteAllText($path, $json, (New-Object Text.UTF8Encoding $false))
    return $path
}
