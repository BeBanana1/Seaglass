<#
.SYNOPSIS
  Construit, signe et (sur demande) publie une version de Seaglass.

.DESCRIPTION
  1. Vérifie l'arbre git, `.env`, la clé de signature et la version (`VERSION`).
  2. Copie mpv dans `src-tauri/binaries/` (embarqué via `tauri.release.conf.json`), s'il
     correspond à `scripts/mpv.lock` (version + SHA-256).
  3. Vérifie le mot de passe de la clé, puis construit l'installeur NSIS signé pour
     l'updater (`pnpm tauri build`).
  4. Range les fichiers à publier dans `release/` :
       release/latest.json                       manifeste lu par l'updater
       release/windows.json                      read by the website's download page (#517)
       release/<version>/Seaglass_<version>_x64-setup.exe
  5. Avec -Publish : copie `release/` vers SEAGLASS_RELEASE_DEST, pose et pousse le tag
     git `v<version>` (sauf s'il est déjà sur HEAD : release Android), puis crée la release
     GitHub du tag (`github-release.ps1`, sauf avec -SkipGitHubRelease).

  -PublishOnly saute les étapes 2 à 4 et publie le dernier build, s'il a été fait sur ce
  commit, arbre propre (`release-all.ps1` construit Windows et Android avant de publier).

  Variables d'environnement :
    SEAGLASS_UPDATE_BASE_URL  URL publique du dossier de mises à jour (requise ; environnement ou .env)
    SEAGLASS_UPDATER_PUBKEY   clé publique de l'updater (défaut : <clé privée>.pub)
    SEAGLASS_RELEASE_DEST     destination de publication : dossier local ou UNC, ou cible
                              scp « utilisateur@hôte:/chemin » ou « alias-ssh:/chemin »
    SEAGLASS_MPV_PATH         mpv.exe à embarquer (défaut : celui du PATH), conforme à mpv.lock ;
                              copie figée posée par update-mpv-lock.ps1
    SEAGLASS_SIGNING_KEY      clé privée de l'updater (défaut : ~/.tauri/seaglass.key)
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD
                              mot de passe de la clé ; sinon entrée générique
                              « seaglass-signing-key » du Gestionnaire d'identifiants,
                              sinon demandé au clavier
    SEAGLASS_GITHUB_TOKEN     jeton GitHub de la release (voir github-release.ps1)

.EXAMPLE
  pnpm release                      # construit dans release/, ne publie rien
  pnpm release -Publish             # construit, publie et tague
  pnpm release -PublishOnly         # publie le dernier build (fait sur ce commit)
  pnpm release -Notes "- Correctif lecture"
#>
param(
    [switch]$Publish,
    [string]$Notes,
    # Pour tester un build avec des modifications non commitées (refusé avec -Publish).
    # L'installeur d'un tel build n'est pas compressé (makensis ~1 s au lieu de ~50 s).
    [switch]$AllowDirty,
    # Publie le dernier build au lieu d'en refaire un (implique -Publish).
    [switch]$PublishOnly,
    # La release GitHub est faite par l'appelant (release-all.ps1).
    [switch]$SkipGitHubRelease
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Fail($message) { Write-Host "ERREUR : $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }
. (Join-Path $PSScriptRoot 'build-stamp.ps1')
. (Join-Path $PSScriptRoot 'updater-manifest.ps1')
. (Join-Path $PSScriptRoot 'release-notes.ps1')
if ($PublishOnly) { $Publish = $true }

# --- 1. Vérifications -------------------------------------------------------------------
$version = (Get-Content VERSION -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { Fail "VERSION invalide : '$version'" }
$tag = "v$version"

$dirty = git status --porcelain
if ($dirty -and ($Publish -or -not $AllowDirty)) {
    Fail "des modifications ne sont pas commitées (utiliser -AllowDirty pour un build de test)"
}
# Le tag peut déjà avoir été posé sur ce commit par la release Android (release-android.ps1).
$tagExists = [bool](git tag --list $tag)
if ($Publish) {
    if ($tagExists -and (git rev-parse "$tag^{commit}") -ne (git rev-parse HEAD)) {
        Fail "le tag $tag existe sur un autre commit : monter VERSION avant de publier"
    }
    if (-not $env:SEAGLASS_RELEASE_DEST) { Fail "SEAGLASS_RELEASE_DEST n'est pas défini" }
}
if (-not (Test-Path .env)) { Fail ".env absent : l'identifiant client est injecté à la compilation" }

$keyPath = if ($env:SEAGLASS_SIGNING_KEY) { $env:SEAGLASS_SIGNING_KEY } else { Join-Path $HOME '.tauri\seaglass.key' }
if (-not (Test-Path $keyPath)) { Fail "clé de signature introuvable : $keyPath" }

# Même adresse compilée dans l'app (#452, lue par build.rs) et écrite dans les manifestes ;
# clé publique compilée elle aussi (#489). Aucune valeur par défaut.
. (Join-Path $PSScriptRoot 'release-env.ps1')
$baseUrl = Use-UpdateBaseUrl
Use-UpdaterPubkey $keyPath

if (-not $PublishOnly) {
    # --- 2. mpv embarqué --------------------------------------------------------------------
    # Seul le mpv décrit par scripts/mpv.lock (version + SHA-256, versionnés) part dans
    # l'installeur : un mpv remplacé ou mis à jour sur ce PC serait sinon distribué signé.
    Step "mpv"
    $lock = @{}
    Get-Content (Join-Path $PSScriptRoot 'mpv.lock') -Encoding UTF8 | Where-Object { $_ -match '^\s*[^#\s]' } | ForEach-Object {
        $name, $value = $_ -split '=', 2
        $lock[$name.Trim()] = "$value".Trim()
    }
    if (-not $lock.sha256 -or -not $lock.version) { Fail "scripts/mpv.lock incomplet (version et sha256 attendus)" }
    $mpvSource = if ($env:SEAGLASS_MPV_PATH) { $env:SEAGLASS_MPV_PATH } else { (Get-Command mpv.exe -ErrorAction SilentlyContinue).Source }
    if (-not $mpvSource -or -not (Test-Path $mpvSource)) { Fail "mpv.exe introuvable (PATH ou SEAGLASS_MPV_PATH)" }
    # Empreinte vérifiée avant d'exécuter quoi que ce soit de ce binaire.
    $mpvHash = (Get-FileHash $mpvSource -Algorithm SHA256).Hash
    if ($mpvHash -ne $lock.sha256.ToUpperInvariant()) {
        Fail ("mpv.exe ne correspond pas à scripts/mpv.lock`n    fichier  : $mpvSource`n    SHA-256  : $mpvHash`n" +
            "    attendu  : $($lock.sha256)`n    mpv a changé (mise à jour, remplacement ?) : vérifier ce binaire, puis le figer et`n" +
            "    mettre à jour mpv.lock avec « powershell -File scripts/update-mpv-lock.ps1 » (à commiter)")
    }
    $mpvVersion = ((& $mpvSource --version | Select-Object -First 1) -split '\s+' | Select-Object -First 2) -join ' '
    if ($mpvVersion -ne $lock.version) { Fail "version de mpv inattendue : '$mpvVersion' (mpv.lock : '$($lock.version)')" }
    $sidecar = 'src-tauri\binaries\mpv-x86_64-pc-windows-msvc.exe'
    New-Item -ItemType Directory -Force (Split-Path $sidecar) | Out-Null
    Copy-Item $mpvSource $sidecar -Force
    if ((Get-FileHash $sidecar -Algorithm SHA256).Hash -ne $mpvHash) { Fail "copie de mpv altérée : $sidecar" }
    Write-Host "    $mpvVersion, SHA-256 conforme à mpv.lock"

    # --- 3. Build signé ---------------------------------------------------------------------
    Step "clé de signature"
    . (Join-Path $PSScriptRoot 'signing-key.ps1')
    $keyPassword = Get-SigningKeyPassword $keyPath

    Step "build de Seaglass $version"
    $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
    # Vide = variable absente sous PowerShell 5.1 : Tauri suppose alors un mot de passe vide.
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $keyPassword
    try {
        $updaterConfig = New-UpdaterConfig
        $configs = @('--config', (Join-Path $root 'src-tauri\tauri.release.conf.json'), '--config', $updaterConfig)
        if ($dirty) {
            # Build de test, jamais publiable (-PublishOnly refuse un arbre modifié) : la
            # compression LZMA de mpv (~50 s, un seul cœur) ne sert qu'aux téléchargements.
            Write-Host "    build de test : installeur non compressé (~140 Mo)" -ForegroundColor Yellow
            $configs += @('--config', (Join-Path $PSScriptRoot 'tauri.test.conf.json'))
        }
        pnpm tauri build @configs
        $buildExit = $LASTEXITCODE
    } finally {
        Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
        $keyPassword = $null
        if ($updaterConfig) { Remove-Item $updaterConfig -ErrorAction SilentlyContinue }
    }
    if ($buildExit -ne 0) { Fail "échec du build" }

    $bundle = 'src-tauri\target\release\bundle\nsis'
    $setupName = "Seaglass_${version}_x64-setup.exe"
    $setup = Join-Path $bundle $setupName
    if (-not (Test-Path $setup) -or -not (Test-Path "$setup.sig")) { Fail "installeur ou signature absents dans $bundle" }

    # --- 4. Fichiers à publier --------------------------------------------------------------
    Step "manifeste"
    $out = Join-Path $root "release\$version"
    New-Item -ItemType Directory -Force $out | Out-Null
    Copy-Item $setup, "$setup.sig" $out -Force

    # From CHANGELOG.md (#49): latest.json is shared with Linux, windows.json feeds the website.
    if (-not $Notes) { $Notes = Get-ReleaseNotes $version 'desktop' }
    # Repère avant le manifeste : il décide des plateformes annoncées (Linux s'il a été
    # construit sur ce commit, #314).
    Write-BuildStamp 'windows' $version
    $announced = Write-UpdaterManifest $version $Notes $baseUrl
    $manifestPath = Join-Path $root 'release\latest.json'

    $item = Get-Item $setup
    $size = [math]::Round($item.Length / 1MB, 1)

    # Manifest of the website's download page (#517), same format as android.json: the page
    # is static (/download/ page of site/) and reads it from the updates folder.
    $download = [ordered]@{
        version  = $version
        url      = "$baseUrl/$version/$setupName"
        sha256   = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLowerInvariant()
        size     = $item.Length
        notes    = $Notes
        pub_date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    }
    $downloadPath = Join-Path $root 'release\windows.json'
    [System.IO.File]::WriteAllText($downloadPath, ($download | ConvertTo-Json -Depth 3), [System.Text.UTF8Encoding]::new($false))

    Write-Host "    installeur : release\$version\$setupName ($size Mo)"
    Write-Host "    manifeste  : release\latest.json -> $baseUrl/latest.json ($($announced -join ', '))"
    Write-Host "    page       : release\windows.json -> $baseUrl/windows.json"
} else {
    $setupName = "Seaglass_${version}_x64-setup.exe"
    $out = Join-Path $root "release\$version"
    $manifestPath = Join-Path $root 'release\latest.json'
    $downloadPath = Join-Path $root 'release\windows.json'
    Assert-BuildStamp 'windows' $version @((Join-Path $out $setupName), (Join-Path $out "$setupName.sig"), $manifestPath, $downloadPath)
    if ((Get-Content $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json).version -ne $version) {
        Fail "release\latest.json ne décrit pas la $version"
    }
    Write-Host "    build $version de ce commit repris"
}

# --- 5. Publication ---------------------------------------------------------------------
if (-not $Publish) {
    Write-Host "Build terminé, rien n'a été publié (ajouter -Publish)." -ForegroundColor Green
    exit 0
}

Step "publication vers $env:SEAGLASS_RELEASE_DEST"
$dest = $env:SEAGLASS_RELEASE_DEST
# latest.json peut aussi annoncer l'AppImage (#314) : elle doit déjà être en ligne.
Assert-ManifestOnline $manifestPath $dest @('windows-x86_64')
# Le manifeste part en dernier : l'updater ne voit la version qu'une fois l'installeur en place.
# Cible scp : « hôte:… » ou « utilisateur@hôte:… » (au moins 2 caractères avant « : »,
# pour ne pas confondre avec une lettre de lecteur comme « D: »).
if ($dest -match '^[^\\/:]{2,}:') {
    $sshHost, $remotePath = $dest -split ':', 2
    # Seulement les fichiers Windows : release/<version>/ peut aussi contenir l'APK.
    ssh -n $sshHost "mkdir -p '$remotePath/$version'"
    if ($LASTEXITCODE -ne 0) { Fail "échec de la création du dossier de version" }
    scp (Join-Path $out $setupName) (Join-Path $out "$setupName.sig") "$dest/$version/"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (installeur)" }
    # scp reprend des droits Windows trop larges (dossier en écriture pour tous) : on les
    # remet à 755 / 644 avant de rendre la version visible.
    ssh -n $sshHost "chmod 755 '$remotePath/$version' && chmod 644 '$remotePath/$version'/*"
    if ($LASTEXITCODE -ne 0) { Fail "échec du réglage des droits sur le serveur" }
    # The download page's manifest (#517), then the updater's last.
    scp $downloadPath "$dest/windows.json"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (windows.json)" }
    scp $manifestPath "$dest/latest.json"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (manifeste)" }
    ssh -n $sshHost "chmod 644 '$remotePath/windows.json' '$remotePath/latest.json'"
} else {
    New-Item -ItemType Directory -Force $dest | Out-Null
    New-Item -ItemType Directory -Force (Join-Path $dest $version) | Out-Null
    Copy-Item (Join-Path $out $setupName), (Join-Path $out "$setupName.sig") (Join-Path $dest $version) -Force
    Copy-Item $downloadPath (Join-Path $dest 'windows.json') -Force
    Copy-Item $manifestPath (Join-Path $dest 'latest.json') -Force
}

if ($tagExists) {
    Write-Host "    tag $tag déjà posé sur HEAD (release Android)"
} else {
    Step "tag $tag"
    git tag -a $tag -m "Seaglass $version"
    git push origin $tag
    if ($LASTEXITCODE -ne 0) { Fail "échec du push du tag" }
}
Write-Host "Seaglass $version publiée." -ForegroundColor Green
if ($SkipGitHubRelease) { exit 0 }

# La mise à jour est déjà en ligne : un échec ici n'est qu'un avertissement (#108).
try {
    & (Join-Path $PSScriptRoot 'github-release.ps1') -Version $version
    $githubOk = $LASTEXITCODE -eq 0
} catch {
    Write-Host "ERREUR : $_" -ForegroundColor Red
    $githubOk = $false
}
if (-not $githubOk) {
    Write-Host ("ATTENTION : release GitHub non créée, la mise à jour reste publiée.`n" +
        "    Pour la refaire seule : powershell -File scripts/github-release.ps1") -ForegroundColor Yellow
}
