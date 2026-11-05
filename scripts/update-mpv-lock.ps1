<#
.SYNOPSIS
  Fige un mpv pour les releases et met scripts/mpv.lock à jour en une étape.

.DESCRIPTION
  Le mpv de tous les jours (zhongfly, mis à jour par son updater.bat) change souvent ; celui
  des releases ne doit changer que sur décision (#87). Ce script :
    1. prend le mpv.exe source (défaut : celui du PATH) ;
    2. en affiche le chemin, la taille, la date et l'empreinte, et l'ancien verrou ;
    3. après confirmation, le copie dans un dossier figé (défaut :
       %LOCALAPPDATA%\Seaglass-build\mpv.exe), que l'updater de zhongfly ne touche pas ;
    4. vérifie l'empreinte de la copie, puis lit sa version (seule la copie est exécutée) ;
    5. réécrit scripts/mpv.lock et pose la variable utilisateur SEAGLASS_MPV_PATH sur la
       copie, que release.ps1 lit en priorité.
  Rien n'est commité : la commande git à lancer est affichée à la fin.

.EXAMPLE
  powershell -File scripts/update-mpv-lock.ps1
  powershell -File scripts/update-mpv-lock.ps1 -Source "C:\Program Files\mpv\mpv.exe"
#>
param(
    # mpv.exe à figer (défaut : celui du PATH).
    [string]$Source,
    # Emplacement de la copie figée (défaut : %LOCALAPPDATA%\Seaglass-build\mpv.exe).
    [string]$Destination = (Join-Path $env:LOCALAPPDATA 'Seaglass-build\mpv.exe'),
    # Ne pas demander de confirmation (le binaire a déjà été vérifié).
    [switch]$Yes
)

$ErrorActionPreference = 'Stop'
function Fail($message) { Write-Host "ERREUR : $message" -ForegroundColor Red; exit 1 }

$lockPath = Join-Path $PSScriptRoot 'mpv.lock'

# --- Source -----------------------------------------------------------------------------
if (-not $Source) {
    $Source = (Get-Command mpv.exe -ErrorAction SilentlyContinue).Source
    if (-not $Source) { Fail "mpv.exe introuvable dans le PATH (préciser -Source)" }
}
if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) { Fail "fichier introuvable : $Source" }
$Source = (Resolve-Path -LiteralPath $Source).Path
$Destination = [IO.Path]::GetFullPath($Destination)
$file = Get-Item -LiteralPath $Source
$hash = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash

# --- Ancien verrou ----------------------------------------------------------------------
$old = @{}
if (Test-Path $lockPath) {
    Get-Content $lockPath -Encoding UTF8 | Where-Object { $_ -match '^\s*[^#\s]' } | ForEach-Object {
        $name, $value = $_ -split '=', 2
        $old[$name.Trim()] = "$value".Trim()
    }
}

Write-Host "mpv à figer pour les releases"
Write-Host "    fichier  : $Source"
Write-Host ("    taille   : {0:N1} Mo, modifié le {1:yyyy-MM-dd HH:mm}" -f ($file.Length / 1MB), $file.LastWriteTime)
Write-Host "    SHA-256  : $hash"
Write-Host "    verrou   : $($old.version), $($old.sha256)"
Write-Host "    copie    : $Destination"
if ($old.sha256 -and $hash -eq $old.sha256.ToUpperInvariant() -and $Source -eq $Destination) {
    Write-Host "Déjà figé et conforme à mpv.lock : rien à faire." -ForegroundColor Green
    exit 0
}

if (-not $Yes) {
    $answer = Read-Host "Figer ce binaire et l'embarquer dans les prochaines releases ? (o/N)"
    if ($answer -notmatch '^(o|oui|y|yes)$') { Write-Host "Annulé, rien n'a changé."; exit 1 }
}

# --- Copie figée ------------------------------------------------------------------------
# Copiée à côté puis renommée : une copie interrompue ne remplace jamais la précédente.
if ($Source -ne $Destination) {
    New-Item -ItemType Directory -Force (Split-Path $Destination) | Out-Null
    $partial = "$Destination.partial"
    Copy-Item -LiteralPath $Source $partial -Force
    if ((Get-FileHash -LiteralPath $partial -Algorithm SHA256).Hash -ne $hash) {
        Remove-Item -LiteralPath $partial -Force
        Fail "la copie ne correspond pas à la source (fichier modifié pendant la copie ?)"
    }
    Move-Item -LiteralPath $partial $Destination -Force
}
if ((Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash -ne $hash) { Fail "copie figée altérée : $Destination" }

# Version lue sur la copie vérifiée, jamais sur la source (qui peut changer entre-temps).
$version = ((& $Destination --version | Select-Object -First 1) -split '\s+' | Select-Object -First 2) -join ' '
if ($version -notmatch '^mpv \S+$') { Fail "version illisible : '$version'" }

# --- mpv.lock ---------------------------------------------------------------------------
$lock = @(
    "# mpv embarqué par l'installeur (#87). ``release.ps1`` refuse tout autre binaire.",
    "# Généré par ``scripts/update-mpv-lock.ps1``, qui fige aussi la copie lue par release.ps1",
    "# (SEAGLASS_MPV_PATH) : vérifier le nouveau mpv, lancer le script, commiter ce fichier.",
    "# Build x86-64-v3 (AVX2 exigé, #92) de zhongfly, relevée le $(Get-Date -Format 'yyyy-MM-dd').",
    "version=$version",
    "sha256=$hash",
    ""
) -join "`n"
[IO.File]::WriteAllText($lockPath, $lock, (New-Object Text.UTF8Encoding $false))

# --- SEAGLASS_MPV_PATH ------------------------------------------------------------------
if ([Environment]::GetEnvironmentVariable('SEAGLASS_MPV_PATH', 'User') -ne $Destination) {
    [Environment]::SetEnvironmentVariable('SEAGLASS_MPV_PATH', $Destination, 'User')
    Write-Host "SEAGLASS_MPV_PATH (variable utilisateur) = $Destination"
    Write-Host "    effective dans les nouveaux terminaux ; ici : `$env:SEAGLASS_MPV_PATH = '$Destination'"
}

Write-Host "mpv.lock mis à jour : $version" -ForegroundColor Green
Write-Host "    à commiter : git commit -m `"chore(release): mpv $($version -replace '^mpv ', '')`" scripts/mpv.lock"
