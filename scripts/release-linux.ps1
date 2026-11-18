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

  L'AppImage est signée pour l'updater (#314) avec la clé de l'installeur Windows (même
  mot de passe, `signing-key.ps1`), transmise à WSL par l'environnement. Le build réécrit
  `release/latest.json` (`updater-manifest.ps1`) : il annonce l'AppImage, et l'installeur
  Windows s'il a été construit sur ce commit. Ce manifeste n'est envoyé que par la
  publication Windows, en dernier (`release-all.ps1` publie Linux avant) : publiée seule,
  l'AppImage est téléchargeable mais pas encore proposée aux mises à jour.

  La distribution de build se prépare une fois avec `scripts/linux-build-setup.sh` (voir
  docs/linux.md, « Release »).

  Variables d'environnement :
    SEAGLASS_LINUX_DISTRO      distribution WSL de build (défaut : Ubuntu-22.04)
    SEAGLASS_LINUX_GLIBC_MAX   glibc la plus récente admise (défaut : 2.35, celle d'Ubuntu 22.04)
    SEAGLASS_UPDATE_BASE_URL   URL publique du dossier de mises à jour (requise ; environnement ou .env)
    SEAGLASS_UPDATER_PUBKEY    clé publique de l'updater (défaut : <clé privée>.pub)
    SEAGLASS_RELEASE_DEST      destination de publication (voir release.ps1)
    SEAGLASS_SIGNING_KEY       clé privée de l'updater (défaut : ~/.tauri/seaglass.key)
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD
                               mot de passe de la clé (voir release.ps1)

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
. (Join-Path $PSScriptRoot 'updater-manifest.ps1')
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

# Même adresse compilée dans l'app (#452, lue par build.rs, passée à WSL) et écrite dans
# le manifeste. Aucune valeur par défaut (#489).
. (Join-Path $PSScriptRoot 'release-env.ps1')
$baseUrl = Use-UpdateBaseUrl
$imageName = "Seaglass_${version}_amd64.AppImage"
$out = Join-Path $root "release\$version"
$imageOut = Join-Path $out $imageName
$sigOut = "$imageOut.sig"
$manifestPath = Join-Path $root 'release\linux.json'

if (-not $PublishOnly) {
    if (-not (Test-Path .env)) { Fail ".env absent : l'identifiant client est injecté à la compilation" }
    $distro = if ($env:SEAGLASS_LINUX_DISTRO) { $env:SEAGLASS_LINUX_DISTRO } else { 'Ubuntu-22.04' }
    $glibcMax = if ($env:SEAGLASS_LINUX_GLIBC_MAX) { $env:SEAGLASS_LINUX_GLIBC_MAX } else { '2.35' }
    if ($glibcMax -notmatch '^\d+\.\d+$') { Fail "SEAGLASS_LINUX_GLIBC_MAX invalide : '$glibcMax'" }
    $keyPath = if ($env:SEAGLASS_SIGNING_KEY) { $env:SEAGLASS_SIGNING_KEY } else { Join-Path $HOME '.tauri\seaglass.key' }
    if (-not (Test-Path $keyPath)) { Fail "clé de signature introuvable : $keyPath" }
    Use-UpdaterPubkey $keyPath

    # `wsl -l -q` écrit en UTF-16 : les caractères nuls sont retirés avant la comparaison.
    $distros = @(wsl.exe -l -q | ForEach-Object { ($_ -replace "`0", '').Trim() } | Where-Object { $_ })
    if ($distros -notcontains $distro) {
        Fail ("distribution WSL « $distro » absente. La créer puis la préparer :`n" +
            "    wsl --install -d $distro`n" +
            "    wsl -d $distro -u root bash scripts/linux-build-setup.sh --system`n" +
            "    wsl -d $distro bash scripts/linux-build-setup.sh")
    }

    Step "clé de signature"
    . (Join-Path $PSScriptRoot 'signing-key.ps1')
    $keyPassword = Get-SigningKeyPassword $keyPath

    # --- 2. Build ---------------------------------------------------------------------------
    Step "frontend"
    pnpm build
    if ($LASTEXITCODE -ne 0) { Fail "échec du build du frontend" }

    Step "AppImage de Seaglass $version ($distro, glibc <= $glibcMax)"
    $toWsl = { param($path) (wsl.exe -d $distro wslpath -a ($path -replace '\\', '/')).Trim() }
    New-Item -ItemType Directory -Force $out | Out-Null
    Remove-Item $imageOut, $sigOut -ErrorAction SilentlyContinue
    $wslRoot = & $toWsl $root
    $wslOut = & $toWsl $out
    # Seuls les fichiers du dépôt partent dans WSL (#418) : lire /mnt/g y coûte cher par
    # fichier, et l'arbre contient des dizaines de milliers de fichiers inutiles au build
    # (worktrees, `relay/target`, builds Android). Fichiers suivis et non suivis non
    # ignorés (build de test avec -AllowDirty), sauf ceux supprimés, plus `.env` et `dist/`.
    $buildDir = Join-Path $root 'release\.build'
    New-Item -ItemType Directory -Force $buildDir | Out-Null
    $fileList = Join-Path $buildDir 'linux-files.txt'
    $files = @(git -c core.quotepath=off ls-files --cached --others --exclude-standard | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
    if ($LASTEXITCODE -ne 0 -or -not $files) { Fail "liste des fichiers du dépôt impossible à obtenir" }
    $files += '.env'
    $files += @(Get-ChildItem dist -Recurse -File | ForEach-Object { (Resolve-Path -Relative $_.FullName).Substring(2) -replace '\\', '/' })
    [System.IO.File]::WriteAllText($fileList, (($files | Sort-Object -Unique) -join "`n") + "`n", [System.Text.UTF8Encoding]::new($false))
    $wslFileList = & $toWsl $fileList
    # Clé et mot de passe passés à WSL par l'environnement (WSLENV), jamais par un fichier ;
    # retirés dès la fin du build.
    $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $keyPath -Raw).Trim()
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $keyPassword
    $savedWslEnv = $env:WSLENV
    $env:WSLENV = (@($env:WSLENV, 'TAURI_SIGNING_PRIVATE_KEY', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'SEAGLASS_UPDATE_BASE_URL', 'SEAGLASS_UPDATER_PUBKEY') | Where-Object { $_ }) -join ':'
    try {
        # Sortie affichée telle quelle (pas capturée) ; le nom de l'AppImage est connu d'avance.
        wsl.exe -d $distro --cd $wslRoot bash scripts/linux-build.sh $wslRoot $wslOut $glibcMax $wslFileList
        $buildExit = $LASTEXITCODE
    } finally {
        Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
        if ($savedWslEnv) { $env:WSLENV = $savedWslEnv } else { Remove-Item Env:WSLENV -ErrorAction SilentlyContinue }
        $keyPassword = $null
    }
    if ($buildExit -ne 0) { Fail "échec du build Linux" }
    if (-not (Test-Path $imageOut) -or -not (Test-Path $sigOut)) { Fail "AppImage ou signature absentes dans release\$version" }

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
    Write-BuildStamp 'linux' $version
    $announced = Write-UpdaterManifest $version $Notes $baseUrl
    Write-Host "    AppImage  : release\$version\$imageName ($size Mo)"
    Write-Host "    manifeste : release\linux.json -> $baseUrl/linux.json"
    Write-Host "    updater   : release\latest.json ($($announced -join ', ')), envoyé par la publication Windows"
} else {
    Assert-BuildStamp 'linux' $version @($imageOut, $sigOut, $manifestPath)
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
    scp $imageOut $sigOut "$dest/$version/"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (AppImage)" }
    scp $manifestPath "$dest/linux.json"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (manifeste)" }
    ssh $sshHost "chmod 644 '$remotePath/$version/$imageName' '$remotePath/$version/$imageName.sig' '$remotePath/linux.json'"
    if ($LASTEXITCODE -ne 0) { Fail "échec du réglage des droits sur le serveur" }
} else {
    New-Item -ItemType Directory -Force (Join-Path $dest $version) | Out-Null
    Copy-Item $imageOut, $sigOut (Join-Path $dest $version) -Force
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
Write-Host "    mises à jour : proposées une fois latest.json envoyé par la publication Windows (release.ps1)"
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
