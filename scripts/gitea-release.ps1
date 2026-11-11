<#
.SYNOPSIS
  Crée (ou complète) la release Gitea d'une version déjà publiée (#108).

.DESCRIPTION
  Appelé par `release.ps1`, `release-android.ps1` ou `release-linux.ps1` avec -Publish, ou par `release-all.ps1`
  une fois le tag poussé, ou seul si cette étape a échoué ou pour une version publiée
  avant #108. Rien n'est reconstruit : les pièces jointes sont exactement les fichiers
  publiés sur le serveur de mises à jour, pris dans `release/` (ceux qui existent ; au
  moins l'installeur, l'APK ou l'AppImage) :
    release/<version>/Seaglass_<version>_x64-setup.exe
    release/<version>/Seaglass_<version>_x64-setup.exe.sig   (s'il existe)
    release/latest.json                       (seulement s'il décrit cette version)
    release/<version>/Seaglass_<version>_arm64.apk
    release/android.json                      (seulement s'il décrit cette version)
    release/<version>/Seaglass_<version>_amd64.AppImage
    release/<version>/Seaglass_<version>_amd64.AppImage.sig  (s'il existe)
    release/linux.json                        (seulement s'il décrit cette version)

  La release porte le tag `v<version>` (qui doit déjà exister sur Gitea), le titre
  « Seaglass <version> » et les notes du manifeste (sinon le journal git depuis le tag
  précédent). Si elle existe déjà, ses notes restent et les pièces jointes du même nom
  sont remplacées.

  Variables d'environnement :
    SEAGLASS_GITEA_TOKEN  jeton Gitea de l'utilisateur (droit write:repository) ; sinon
                          entrée générique « seaglass-gitea-token » du Gestionnaire
                          d'identifiants (cmdkey /generic:seaglass-gitea-token /user:seaglass /pass)
    SEAGLASS_GITEA_URL    défaut : https://git.bebanana.ch
    SEAGLASS_GITEA_REPO   défaut : bebanana/Seaglass

.EXAMPLE
  powershell -File scripts/gitea-release.ps1   # version de VERSION
  powershell -File scripts/gitea-release.ps1 -Version 0.4.7
#>
param([string]$Version)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
. (Join-Path $PSScriptRoot 'credentials.ps1')

function Fail($message) { Write-Host "ERREUR : $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }

if (-not $Version) { $Version = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim() }
if ($Version -notmatch '^\d+\.\d+\.\d+$') { Fail "version invalide : '$Version'" }
$tag = "v$Version"
$giteaUrl = if ($env:SEAGLASS_GITEA_URL) { $env:SEAGLASS_GITEA_URL.TrimEnd('/') } else { 'https://git.bebanana.ch' }
$repo = if ($env:SEAGLASS_GITEA_REPO) { $env:SEAGLASS_GITEA_REPO } else { 'bebanana/Seaglass' }
$api = "$giteaUrl/api/v1/repos/$repo"

Step "release Gitea $tag"

# --- Fichiers ---------------------------------------------------------------------------
# Un manifeste n'est joint qu'à la version qu'il décrit (la dernière construite) ; ses
# notes servent à la release. Pour une version plus ancienne, notes tirées du journal git.
function Read-Manifest([string]$name) {
    $path = Join-Path $root "release\$name"
    if (-not (Test-Path $path)) { return $null }
    $manifest = Get-Content $path -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($manifest.version -ne $Version) {
        Write-Host "    release\$name ne décrit pas la $Version : non joint"
        return $null
    }
    @{ path = $path; notes = $manifest.notes }
}

$files = @()
$notes = $null
$setup = Join-Path $root "release\$Version\Seaglass_${Version}_x64-setup.exe"
if (Test-Path $setup) {
    $files += $setup
    # Les versions antérieures à #108 n'ont pas gardé la signature dans release/.
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
    # Signature de l'updater, depuis #314.
    if (Test-Path "$appImage.sig") { $files += "$appImage.sig" }
    $linux = Read-Manifest 'linux.json'
    if ($linux) {
        $files += $linux.path
        if (-not $notes) { $notes = $linux.notes }
    }
}
if (-not $files) { Fail "ni installeur, ni APK, ni AppImage de la $Version dans release\" }

if (-not $notes) {
    Write-Host "    notes tirées de git"
    $tags = @(git tag --list 'v*' --sort=v:refname)
    $index = [array]::IndexOf($tags, $tag)
    $range = if ($index -gt 0) { "$($tags[$index - 1])..$tag" } else { $tag }
    $notes = (git log $range --no-merges --pretty=format:'- %s' -n 30) -join "`n"
}

# Sans le tag sur Gitea, l'API en créerait un sur la branche par défaut.
$remoteTag = git ls-remote --tags origin "refs/tags/$tag"
if ($LASTEXITCODE -ne 0) { Fail "impossible de lire les tags de origin" }
if (-not $remoteTag) { Fail "le tag $tag n'est pas sur origin (le pousser d'abord)" }

# --- Jeton ------------------------------------------------------------------------------
# Celui de l'utilisateur : la release est à son nom. Jamais affiché ni écrit.
$token = $env:SEAGLASS_GITEA_TOKEN
if (-not $token) { $token = Read-StoredSecret 'seaglass-gitea-token' }
if (-not $token) {
    Fail "jeton Gitea absent : définir SEAGLASS_GITEA_TOKEN ou l'entrée « seaglass-gitea-token » du Gestionnaire d'identifiants"
}

# --- API --------------------------------------------------------------------------------
# HttpClient plutôt qu'Invoke-RestMethod : PowerShell 5.1 n'a pas -Form pour les pièces jointes.
Add-Type -AssemblyName System.Net.Http
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$http = [System.Net.Http.HttpClient]::new()
$http.Timeout = [TimeSpan]::FromMinutes(10)
$http.DefaultRequestHeaders.Authorization = [System.Net.Http.Headers.AuthenticationHeaderValue]::new('token', $token)
$token = $null

# Réponse décodée, $null sur un 404 (si -AllowMissing) ; toute autre erreur est fatale.
function Invoke-Gitea([string]$method, [string]$path, $content, [switch]$AllowMissing) {
    $request = [System.Net.Http.HttpRequestMessage]::new([System.Net.Http.HttpMethod]::new($method), "$api$path")
    $request.Headers.Accept.ParseAdd('application/json')
    if ($content) { $request.Content = $content }
    try { $response = $http.SendAsync($request).GetAwaiter().GetResult() }
    catch {
        $cause = $_.Exception.GetBaseException().Message
        Fail "Gitea injoignable ($giteaUrl) : $cause"
    }
    $body = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
    $status = [int]$response.StatusCode
    if ($AllowMissing -and $status -eq 404) { return $null }
    if (-not $response.IsSuccessStatusCode) {
        Fail "$method $path : HTTP $status $($body.Substring(0, [math]::Min(300, $body.Length)))"
    }
    # Entre parenthèses : sous PowerShell 5.1, ConvertFrom-Json renvoie un tableau JSON comme
    # un seul objet ; ainsi ses éléments sont énumérés.
    if ($body) { (ConvertFrom-Json $body) }
}

function Json($value) {
    $text = $value | ConvertTo-Json -Depth 5
    [System.Net.Http.StringContent]::new($text, [System.Text.Encoding]::UTF8, 'application/json')
}

try {
    $release = Invoke-Gitea GET "/releases/tags/$tag" -AllowMissing
    if ($release) {
        Write-Host "    release existante (#$($release.id)), pièces jointes mises à jour"
    } else {
        $release = Invoke-Gitea POST '/releases' (Json ([ordered]@{
            tag_name   = $tag
            name       = "Seaglass $Version"
            body       = $notes
            draft      = $false
            prerelease = $false
        }))
        Write-Host "    release créée (#$($release.id))"
    }

    $existing = @(Invoke-Gitea GET "/releases/$($release.id)/assets")
    foreach ($file in $files) {
        $name = Split-Path $file -Leaf
        foreach ($old in $existing | Where-Object { $_.name -eq $name }) {
            Invoke-Gitea DELETE "/releases/$($release.id)/assets/$($old.id)" | Out-Null
        }
        $stream = [System.IO.File]::OpenRead($file)
        try {
            $form = [System.Net.Http.MultipartFormDataContent]::new()
            $part = [System.Net.Http.StreamContent]::new($stream)
            $part.Headers.ContentType = [System.Net.Http.Headers.MediaTypeHeaderValue]::new('application/octet-stream')
            $form.Add($part, 'attachment', $name)
            $query = [Uri]::EscapeDataString($name)
            Invoke-Gitea POST "/releases/$($release.id)/assets?name=$query" $form | Out-Null
        } finally { $stream.Dispose() }
        $size = [math]::Round((Get-Item $file).Length / 1MB, 1)
        Write-Host "    $name ($size Mo)"
    }
} finally { $http.Dispose() }

Write-Host "Release Gitea : $giteaUrl/$repo/releases/tag/$tag" -ForegroundColor Green
exit 0
