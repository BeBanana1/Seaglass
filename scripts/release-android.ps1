<#
.SYNOPSIS
  Construit, signe et (sur demande) publie l'APK Android de Seaglass (#24).

.DESCRIPTION
  1. Vérifie l'arbre git, `.env`, la clé de release Android et la version (`VERSION`).
  2. Vérifie le mot de passe de la clé (keytool), puis construit l'APK release arm64
     signé (`pnpm tauri android build --apk --target aarch64`).
  3. Vérifie la signature (apksigner : schéma v2 au moins, pas la clé de debug).
  4. Range les fichiers à publier dans `release/` :
       release/android.json                       manifeste lu par l'app (apk_update.rs)
       release/<version>/Seaglass_<version>_arm64.apk
  5. Avec -Publish : copie l'APK puis le manifeste vers SEAGLASS_RELEASE_DEST, pose le
     tag `v<version>` s'il n'existe pas (même tag que l'installeur Windows : il doit alors
     pointer sur HEAD), puis joint l'APK à la release Gitea du tag (`gitea-release.ps1`,
     sauf avec -SkipGiteaRelease).

  -PublishOnly saute les étapes 2 à 4 et publie le dernier build, s'il a été fait sur ce
  commit, arbre propre (`release-all.ps1` construit Windows et Android avant de publier).

  La clé de release se crée une fois (voir CLAUDE.md, « Android », Distribution) et se
  sauvegarde comme la clé minisign : Android refuse toute mise à jour signée par une
  autre clé que celle de l'app installée.

  Variables d'environnement :
    SEAGLASS_ANDROID_KEYSTORE      keystore PKCS12 (défaut : ~/.android/seaglass-release.jks)
    SEAGLASS_ANDROID_KEY_ALIAS     alias de la clé (défaut : seaglass)
    SEAGLASS_ANDROID_KEY_PASSWORD  mot de passe ; sinon entrée générique
                                   « seaglass-android-key » du Gestionnaire d'identifiants,
                                   sinon demandé au clavier
    SEAGLASS_UPDATE_BASE_URL       URL publique du dossier de mises à jour
                                   (défaut : https://updates.bebanana.ch/seaglass ; l'app
                                   n'accepte que cette adresse)
    SEAGLASS_RELEASE_DEST          destination de publication (voir release.ps1)
    ANDROID_HOME, NDK_HOME         défaut : %LOCALAPPDATA%\Android\Sdk et son NDK le plus récent

.EXAMPLE
  powershell -File scripts/release-android.ps1             # construit dans release/
  powershell -File scripts/release-android.ps1 -Publish    # construit, publie et tague
  powershell -File scripts/release-android.ps1 -PublishOnly  # publie le dernier build
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
if (-not (Test-Path .env)) { Fail ".env absent : l'identifiant client est injecté à la compilation" }

$keystore = if ($env:SEAGLASS_ANDROID_KEYSTORE) { $env:SEAGLASS_ANDROID_KEYSTORE } else { Join-Path $HOME '.android\seaglass-release.jks' }
if (-not (Test-Path $keystore)) { Fail "clé de release Android introuvable : $keystore (création : CLAUDE.md, section Android)" }
$keystore = (Resolve-Path $keystore).Path
$alias = if ($env:SEAGLASS_ANDROID_KEY_ALIAS) { $env:SEAGLASS_ANDROID_KEY_ALIAS } else { 'seaglass' }

$baseUrl = if ($env:SEAGLASS_UPDATE_BASE_URL) { $env:SEAGLASS_UPDATE_BASE_URL.TrimEnd('/') } else { 'https://updates.bebanana.ch/seaglass' }

# Outils Android.
$sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
if (-not (Test-Path $sdk)) { Fail "SDK Android introuvable : $sdk" }
$env:ANDROID_HOME = $sdk
if (-not $env:NDK_HOME) {
    $ndk = Get-ChildItem (Join-Path $sdk 'ndk') -Directory -ErrorAction SilentlyContinue |
        Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    if (-not $ndk) { Fail "NDK introuvable dans $sdk\ndk" }
    $env:NDK_HOME = $ndk.FullName
}
$buildTools = Get-ChildItem (Join-Path $sdk 'build-tools') -Directory -ErrorAction SilentlyContinue |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (-not $buildTools) { Fail "build-tools introuvables dans $sdk\build-tools" }
$apksigner = Join-Path $buildTools.FullName 'apksigner.bat'
$keytool = if ($env:JAVA_HOME -and (Test-Path (Join-Path $env:JAVA_HOME 'bin\keytool.exe'))) {
    Join-Path $env:JAVA_HOME 'bin\keytool.exe'
} else { (Get-Command keytool -ErrorAction SilentlyContinue).Source }
if (-not $keytool) { Fail "keytool introuvable (JAVA_HOME ou PATH)" }

if (-not $PublishOnly) {
    # --- 2. Mot de passe puis build ---------------------------------------------------------
    # Comme pour la clé minisign (#87) : variable de session, sinon Gestionnaire
    # d'identifiants, sinon saisie. Il ne passe que par l'environnement du build.
    Step "clé de release Android"
    . (Join-Path $PSScriptRoot 'credentials.ps1')
    $credentialTarget = 'seaglass-android-key'
    if ($env:SEAGLASS_ANDROID_KEY_PASSWORD) {
        $keyPassword = $env:SEAGLASS_ANDROID_KEY_PASSWORD
        $origin = 'SEAGLASS_ANDROID_KEY_PASSWORD'
    } else {
        $keyPassword = Read-StoredSecret $credentialTarget
        $origin = "Gestionnaire d'identifiants ($credentialTarget)"
    }
    if (-not $keyPassword) {
        $origin = 'saisie'
        try { $secure = Read-Host "    Mot de passe de la clé Android" -AsSecureString }
        catch { Fail "mot de passe de la clé absent (session non interactive) : définir SEAGLASS_ANDROID_KEY_PASSWORD" }
        $keyPassword = [System.Net.NetworkCredential]::new('', $secure).Password
    }
    if (-not $keyPassword) { Fail "mot de passe de la clé vide ($origin)" }

    $env:SEAGLASS_ANDROID_KEYSTORE = $keystore
    $env:SEAGLASS_ANDROID_KEY_ALIAS = $alias
    $env:SEAGLASS_ANDROID_KEY_PASSWORD = $keyPassword
    try {
        # -storepass:env : le mot de passe n'apparaît pas dans la ligne de commande.
        & $keytool -list -keystore $keystore -storetype PKCS12 -alias $alias -storepass:env SEAGLASS_ANDROID_KEY_PASSWORD *> $null
        if ($LASTEXITCODE -ne 0) { Fail "mot de passe ou alias de la clé incorrect ($origin, alias $alias)" }
        Write-Host "    mot de passe vérifié ($origin)"

        Step "build de l'APK $version"
        pnpm tauri android build --apk --target aarch64
        $buildExit = $LASTEXITCODE
    } finally {
        Remove-Item Env:SEAGLASS_ANDROID_KEY_PASSWORD -ErrorAction SilentlyContinue
        $keyPassword = $null
    }
    if ($buildExit -ne 0) { Fail "échec du build" }

    $apk = 'src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release.apk'
    if (-not (Test-Path $apk)) { Fail "APK signé absent : $apk (clé non prise en compte par Gradle ?)" }

    # --- 3. Signature -----------------------------------------------------------------------
    Step "signature"
    $certs = & $apksigner verify --min-sdk-version 24 --print-certs --verbose $apk 2>&1
    if ($LASTEXITCODE -ne 0) { Fail "signature de l'APK invalide :`n$($certs -join "`n")" }
    if (-not ($certs -match 'Verified using v2 scheme \(APK Signature Scheme v2\): true')) { Fail "APK non signé en v2" }
    if ($certs -match 'CN=Android Debug') { Fail "APK signé par la clé de debug" }
    $certSha = ($certs | Select-String 'Signer #1 certificate SHA-256 digest: (.+)$').Matches.Groups[1].Value
    Write-Host "    certificat SHA-256 : $certSha"

    # --- 4. Fichiers à publier --------------------------------------------------------------
    Step "manifeste"
    $out = Join-Path $root "release\$version"
    New-Item -ItemType Directory -Force $out | Out-Null
    $apkName = "Seaglass_${version}_arm64.apk"
    $apkOut = Join-Path $out $apkName
    Copy-Item $apk $apkOut -Force

    if (-not $Notes) {
        $previous = git tag --list 'v*' --sort=-v:refname | Where-Object { $_ -ne $tag } | Select-Object -First 1
        $range = if ($previous) { "$previous..HEAD" } else { 'HEAD' }
        $Notes = (git log $range --no-merges --pretty=format:'- %s' -n 30) -join "`n"
    }
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
    Write-Host "    APK       : release\$version\$apkName ($size Mo)"
    Write-Host "    manifeste : release\android.json -> $baseUrl/android.json"
    Write-BuildStamp 'android' $version
} else {
    $apkName = "Seaglass_${version}_arm64.apk"
    $apkOut = Join-Path $root "release\$version\$apkName"
    $manifestPath = Join-Path $root 'release\android.json'
    Assert-BuildStamp 'android' $version @($apkOut, $manifestPath)
    $manifest = Get-Content $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($manifest.version -ne $version) { Fail "release\android.json ne décrit pas la $version" }
    if ((Get-FileHash $apkOut -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.sha256) {
        Fail "l'APK ne correspond plus à release\android.json"
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
# Le manifeste part en dernier : l'app ne voit la version qu'une fois l'APK en place.
if ($dest -match '^[^\\/:]{2,}:') {
    $sshHost, $remotePath = $dest -split ':', 2
    ssh $sshHost "mkdir -p '$remotePath/$version' && chmod 755 '$remotePath/$version'"
    if ($LASTEXITCODE -ne 0) { Fail "échec de la création du dossier de version" }
    scp $apkOut "$dest/$version/$apkName"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (APK)" }
    scp $manifestPath "$dest/android.json"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (manifeste)" }
    ssh $sshHost "chmod 644 '$remotePath/$version/$apkName' '$remotePath/android.json'"
    if ($LASTEXITCODE -ne 0) { Fail "échec du réglage des droits sur le serveur" }
} else {
    New-Item -ItemType Directory -Force (Join-Path $dest $version) | Out-Null
    Copy-Item $apkOut (Join-Path $dest "$version\$apkName") -Force
    Copy-Item $manifestPath (Join-Path $dest 'android.json') -Force
}

if ($tagExists) {
    Write-Host "    tag $tag déjà posé sur HEAD (release Windows)"
} else {
    Step "tag $tag"
    git tag -a $tag -m "Seaglass $version"
    git push origin $tag
    if ($LASTEXITCODE -ne 0) { Fail "échec du push du tag" }
}
Write-Host "APK Seaglass $version publié." -ForegroundColor Green
if ($SkipGiteaRelease) { exit 0 }

# L'APK est déjà en ligne : un échec ici n'est qu'un avertissement (#108).
try {
    & (Join-Path $PSScriptRoot 'gitea-release.ps1') -Version $version
    $giteaOk = $LASTEXITCODE -eq 0
} catch {
    Write-Host "ERREUR : $_" -ForegroundColor Red
    $giteaOk = $false
}
if (-not $giteaOk) {
    Write-Host ("ATTENTION : APK non joint à la release Gitea, il reste publié.`n" +
        "    Pour le refaire seul : powershell -File scripts/gitea-release.ps1") -ForegroundColor Yellow
}
