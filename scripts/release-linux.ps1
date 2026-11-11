<#
.SYNOPSIS
  Construit et (sur demande) publie l'AppImage Linux de Seaglass (#313).

.DESCRIPTION
  1. Vérifie l'arbre git, `.env`, la version (`VERSION`) et la distribution WSL de build.
  2. Construit le frontend sous Windows (`pnpm build` : Node 20+ n'est pas dans Ubuntu
     22.04), puis l'AppImage dans la distribution WSL (`linux-build.sh`). Le build échoue
     si l'AppImage exige une glibc plus récente que celle de la distribution de build.
  3. Range les fichiers à publier dans `release/` :
       release/linux.json                          manifeste lu par la page de téléchargement
       release/<version>/Seaglass_<version>_amd64.AppImage
  4. Avec -Publish : copie l'AppImage puis le manifeste vers SEAGLASS_RELEASE_DEST, pose le
     tag `v<version>` s'il n'existe pas (il doit sinon pointer sur HEAD), puis joint
     l'AppImage à la release Gitea du tag (`gitea-release.ps1`, sauf avec -SkipGiteaRelease).

  -PublishOnly saute les étapes 2 et 3 et publie le dernier build, s'il a été fait sur ce
  commit, arbre propre (`release-all.ps1` construit toutes les plateformes avant de publier).

  Pas de signature ici : l'AppImage n'a pas encore de mises à jour automatiques (#314).

  La distribution de build se prépare une fois avec `scripts/linux-build-setup.sh` (voir
  docs/linux.md, « Release »).

  Variables d'environnement :
    SEAGLASS_LINUX_DISTRO      distribution WSL de build (défaut : Ubuntu-22.04)
    SEAGLASS_LINUX_GLIBC_MAX   glibc la plus récente admise (défaut : 2.35, celle d'Ubuntu 22.04)
    SEAGLASS_UPDATE_BASE_URL   URL publique du dossier de mises à jour
                               (défaut : https://updates.bebanana.ch/seaglass)
    SEAGLASS_RELEASE_DEST      destination de publication (voir release.ps1)

.EXAMPLE
  powershell -File scripts/release-linux.ps1               # construit dans release/
  powershell -File scripts/release-linux.ps1 -Publish      # construit, publie et tague
  powershell -File scripts/release-linux.ps1 -PublishOnly  # publie le dernier build
#>
param(
    [switch]$Publish,
    [string]$Notes,
    # Pour tester un build avec des modifications non commitées (refusé avec -Publish).
    [switch]$AllowDirty,
    # Publie le dernier build au lieu d'en refaire un (implique -Publish).
    [switch]$PublishOnly,
    # La release Gitea est faite par l'appelant (release-all.ps1).
    [switch]$SkipGiteaRelease
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Fail($message) { Write-Host "ERREUR : $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }
. (Join-Path $PSScriptRoot 'build-stamp.ps1')
if ($PublishOnly) { $Publish = $true }

# --- 1. Vérifications -------------------------------------------------------------------
$version = (Get-Content VERSION -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { Fail "VERSION invalide : '$version'" }
$tag = "v$version"

$dirty = git status --porcelain
if ($dirty -and ($Publish -or -not $AllowDirty)) {
    Fail "des modifications ne sont pas commitées (utiliser -AllowDirty pour un build de test)"
}
$tagExists = [bool](git tag --list $tag)
if ($Publish) {
    if (-not $env:SEAGLASS_RELEASE_DEST) { Fail "SEAGLASS_RELEASE_DEST n'est pas défini" }
    if ($tagExists -and (git rev-parse "$tag^{commit}") -ne (git rev-parse HEAD)) {
        Fail "le tag $tag existe sur un autre commit : monter VERSION avant de publier"
    }
}

$baseUrl = if ($env:SEAGLASS_UPDATE_BASE_URL) { $env:SEAGLASS_UPDATE_BASE_URL.TrimEnd('/') } else { 'https://updates.bebanana.ch/seaglass' }
$imageName = "Seaglass_${version}_amd64.AppImage"
$out = Join-Path $root "release\$version"
$imageOut = Join-Path $out $imageName
$manifestPath = Join-Path $root 'release\linux.json'

if (-not $PublishOnly) {
    if (-not (Test-Path .env)) { Fail ".env absent : l'identifiant client est injecté à la compilation" }
    $distro = if ($env:SEAGLASS_LINUX_DISTRO) { $env:SEAGLASS_LINUX_DISTRO } else { 'Ubuntu-22.04' }
    $glibcMax = if ($env:SEAGLASS_LINUX_GLIBC_MAX) { $env:SEAGLASS_LINUX_GLIBC_MAX } else { '2.35' }
    if ($glibcMax -notmatch '^\d+\.\d+$') { Fail "SEAGLASS_LINUX_GLIBC_MAX invalide : '$glibcMax'" }

    # `wsl -l -q` écrit en UTF-16 : les caractères nuls sont retirés avant la comparaison.
    $distros = @(wsl.exe -l -q | ForEach-Object { ($_ -replace "`0", '').Trim() } | Where-Object { $_ })
    if ($distros -notcontains $distro) {
        Fail ("distribution WSL « $distro » absente. La créer puis la préparer :`n" +
            "    wsl --install -d $distro`n" +
            "    wsl -d $distro -u root bash scripts/linux-build-setup.sh --system`n" +
            "    wsl -d $distro bash scripts/linux-build-setup.sh")
    }

    # --- 2. Build ---------------------------------------------------------------------------
    Step "frontend"
    pnpm build
    if ($LASTEXITCODE -ne 0) { Fail "échec du build du frontend" }

    Step "AppImage de Seaglass $version ($distro, glibc <= $glibcMax)"
    $toWsl = { param($path) (wsl.exe -d $distro wslpath -a ($path -replace '\\', '/')).Trim() }
    New-Item -ItemType Directory -Force $out | Out-Null
    Remove-Item $imageOut -ErrorAction SilentlyContinue
    $wslRoot = & $toWsl $root
    $wslOut = & $toWsl $out
    # Sortie affichée telle quelle (pas capturée) ; le nom de l'AppImage est connu d'avance.
    wsl.exe -d $distro --cd $wslRoot bash scripts/linux-build.sh $wslRoot $wslOut $glibcMax
    if ($LASTEXITCODE -ne 0) { Fail "échec du build Linux" }
    if (-not (Test-Path $imageOut)) { Fail "AppImage absente : $imageOut" }

    # --- 3. Fichiers à publier --------------------------------------------------------------
    Step "manifeste"
    if (-not $Notes) {
        $previous = git tag --list 'v*' --sort=-v:refname | Where-Object { $_ -ne $tag } | Select-Object -First 1
        $range = if ($previous) { "$previous..HEAD" } else { 'HEAD' }
        $Notes = (git log $range --no-merges --pretty=format:'- %s' -n 30) -join "`n"
    }
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
    Write-Host "    AppImage  : release\$version\$imageName ($size Mo)"
    Write-Host "    manifeste : release\linux.json -> $baseUrl/linux.json"
    Write-BuildStamp 'linux' $version
} else {
    Assert-BuildStamp 'linux' $version @($imageOut, $manifestPath)
    $manifest = Get-Content $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($manifest.version -ne $version) { Fail "release\linux.json ne décrit pas la $version" }
    if ((Get-FileHash $imageOut -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.sha256) {
        Fail "l'AppImage ne correspond plus à release\linux.json"
    }
    Write-Host "    build $version de ce commit repris"
}

# --- 4. Publication ---------------------------------------------------------------------
if (-not $Publish) {
    Write-Host "Build terminé, rien n'a été publié (ajouter -Publish)." -ForegroundColor Green
    exit 0
}

Step "publication vers $env:SEAGLASS_RELEASE_DEST"
$dest = $env:SEAGLASS_RELEASE_DEST
# Le manifeste part en dernier : la page ne propose la version qu'une fois l'AppImage en place.
if ($dest -match '^[^\\/:]{2,}:') {
    $sshHost, $remotePath = $dest -split ':', 2
    ssh $sshHost "mkdir -p '$remotePath/$version' && chmod 755 '$remotePath/$version'"
    if ($LASTEXITCODE -ne 0) { Fail "échec de la création du dossier de version" }
    scp $imageOut "$dest/$version/$imageName"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (AppImage)" }
    scp $manifestPath "$dest/linux.json"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (manifeste)" }
    ssh $sshHost "chmod 644 '$remotePath/$version/$imageName' '$remotePath/linux.json'"
    if ($LASTEXITCODE -ne 0) { Fail "échec du réglage des droits sur le serveur" }
} else {
    New-Item -ItemType Directory -Force (Join-Path $dest $version) | Out-Null
    Copy-Item $imageOut (Join-Path $dest "$version\$imageName") -Force
    Copy-Item $manifestPath (Join-Path $dest 'linux.json') -Force
}

if ($tagExists) {
    Write-Host "    tag $tag déjà posé sur HEAD"
} else {
    Step "tag $tag"
    git tag -a $tag -m "Seaglass $version"
    git push origin $tag
    if ($LASTEXITCODE -ne 0) { Fail "échec du push du tag" }
}
Write-Host "AppImage Seaglass $version publiée." -ForegroundColor Green
if ($SkipGiteaRelease) { exit 0 }

# L'AppImage est déjà en ligne : un échec ici n'est qu'un avertissement (#108).
try {
    & (Join-Path $PSScriptRoot 'gitea-release.ps1') -Version $version
    $giteaOk = $LASTEXITCODE -eq 0
} catch {
    Write-Host "ERREUR : $_" -ForegroundColor Red
    $giteaOk = $false
}
if (-not $giteaOk) {
    Write-Host ("ATTENTION : AppImage non jointe à la release Gitea, elle reste publiée.`n" +
        "    Pour la refaire seule : powershell -File scripts/gitea-release.ps1") -ForegroundColor Yellow
}
