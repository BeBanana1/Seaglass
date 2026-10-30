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
       release/index.html                        page de téléchargement (lien stable)
       release/<version>/Seaglass_<version>_x64-setup.exe
  5. Avec -Publish : copie `release/` vers SEAGLASS_RELEASE_DEST, puis pose et pousse le
     tag git `v<version>`.

  Variables d'environnement :
    SEAGLASS_UPDATE_BASE_URL  URL publique du dossier de mises à jour
                              (défaut : https://updates.bebanana.ch/seaglass)
    SEAGLASS_RELEASE_DEST     destination de publication : dossier local ou UNC, ou cible
                              scp « utilisateur@hôte:/chemin » ou « alias-ssh:/chemin »
    SEAGLASS_MPV_PATH         mpv.exe à embarquer (défaut : celui du PATH), conforme à mpv.lock
    SEAGLASS_SIGNING_KEY      clé privée de l'updater (défaut : ~/.tauri/seaglass.key)
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD
                              mot de passe de la clé ; sinon entrée générique
                              « seaglass-signing-key » du Gestionnaire d'identifiants,
                              sinon demandé au clavier

.EXAMPLE
  pnpm release                      # construit dans release/, ne publie rien
  pnpm release -Publish             # construit, publie et tague
  pnpm release -Notes "- Correctif lecture"
#>
param(
    [switch]$Publish,
    [string]$Notes,
    # Pour tester un build avec des modifications non commitées (refusé avec -Publish).
    [switch]$AllowDirty
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Fail($message) { Write-Host "ERREUR : $message" -ForegroundColor Red; exit 1 }
function Step($message) { Write-Host "==> $message" -ForegroundColor Cyan }

# --- 1. Vérifications -------------------------------------------------------------------
$version = (Get-Content VERSION -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { Fail "VERSION invalide : '$version'" }
$tag = "v$version"

$dirty = git status --porcelain
if ($dirty -and ($Publish -or -not $AllowDirty)) {
    Fail "des modifications ne sont pas commitées (utiliser -AllowDirty pour un build de test)"
}
if ($Publish) {
    if (git tag --list $tag) { Fail "le tag $tag existe déjà : monter VERSION avant de publier" }
    if (-not $env:SEAGLASS_RELEASE_DEST) { Fail "SEAGLASS_RELEASE_DEST n'est pas défini" }
}
if (-not (Test-Path .env)) { Fail ".env absent : l'identifiant client est injecté à la compilation" }

$keyPath = if ($env:SEAGLASS_SIGNING_KEY) { $env:SEAGLASS_SIGNING_KEY } else { Join-Path $HOME '.tauri\seaglass.key' }
if (-not (Test-Path $keyPath)) { Fail "clé de signature introuvable : $keyPath" }

$baseUrl = if ($env:SEAGLASS_UPDATE_BASE_URL) { $env:SEAGLASS_UPDATE_BASE_URL.TrimEnd('/') } else { 'https://updates.bebanana.ch/seaglass' }

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
        "    attendu  : $($lock.sha256)`n    mpv a changé (mise à jour, remplacement ?) : vérifier ce binaire, puis mettre à jour mpv.lock dans un commit")
}
$mpvVersion = ((& $mpvSource --version | Select-Object -First 1) -split '\s+' | Select-Object -First 2) -join ' '
if ($mpvVersion -ne $lock.version) { Fail "version de mpv inattendue : '$mpvVersion' (mpv.lock : '$($lock.version)')" }
$sidecar = 'src-tauri\binaries\mpv-x86_64-pc-windows-msvc.exe'
New-Item -ItemType Directory -Force (Split-Path $sidecar) | Out-Null
Copy-Item $mpvSource $sidecar -Force
if ((Get-FileHash $sidecar -Algorithm SHA256).Hash -ne $mpvHash) { Fail "copie de mpv altérée : $sidecar" }
Write-Host "    $mpvVersion, SHA-256 conforme à mpv.lock"

# --- 3. Build signé ---------------------------------------------------------------------
# Mot de passe de la clé (#87) : TAURI_SIGNING_PRIVATE_KEY_PASSWORD (session), sinon entrée
# générique « seaglass-signing-key » du Gestionnaire d'identifiants, sinon saisi au clavier.
# Il ne passe que par l'environnement du build, jamais par un fichier, et il est vérifié
# avant le build (scripts/protect-signing-key.py --check, mot de passe sur l'entrée standard).
Step "clé de signature"
$credentialTarget = 'seaglass-signing-key'
$keyHelp = "procédure : issue #87 / CLAUDE.md, « Signature »"

function Read-StoredSecret([string]$target) {
    if (-not ('Seaglass.StoredCredential' -as [type])) {
        Add-Type -Namespace Seaglass -Name StoredCredential -MemberDefinition @'
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
private struct CREDENTIAL {
    public int Flags; public int Type; public string TargetName; public string Comment;
    public long LastWritten; public int CredentialBlobSize; public IntPtr CredentialBlob;
    public int Persist; public int AttributeCount; public IntPtr Attributes;
    public string TargetAlias; public string UserName;
}
[DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
private static extern bool CredReadW(string target, int type, int flags, out IntPtr credential);
[DllImport("advapi32.dll")]
private static extern void CredFree(IntPtr credential);
// Secret d'une entrée générique (CRED_TYPE_GENERIC), null si elle n'existe pas.
public static string Read(string target) {
    IntPtr ptr;
    if (!CredReadW(target, 1, 0, out ptr)) return null;
    try {
        var c = (CREDENTIAL)Marshal.PtrToStructure(ptr, typeof(CREDENTIAL));
        return c.CredentialBlobSize == 0 ? "" : Marshal.PtrToStringUni(c.CredentialBlob, c.CredentialBlobSize / 2);
    } finally { CredFree(ptr); }
}
'@
    }
    [Seaglass.StoredCredential]::Read($target)
}

# Code de sortie de --check : 0 = le mot de passe ouvre la clé, 1 = non, 2 = clé illisible.
$python = Get-Command python -ErrorAction SilentlyContinue
function Test-KeyPassword([string]$password) {
    # UTF-8 pour les mots de passe non ASCII (le BOM ajouté par PowerShell 5.1 est retiré).
    $OutputEncoding = [System.Text.UTF8Encoding]::new($false)
    $password | & $python.Source -I (Join-Path $PSScriptRoot 'protect-signing-key.py') --check $keyPath | Out-Null
    $LASTEXITCODE
}

$keyPassword = $null
if (-not $python) {
    Write-Host "    python introuvable : mot de passe non vérifié avant le build" -ForegroundColor Yellow
} else {
    switch (Test-KeyPassword '') {
        0 {
            # Clé encore chiffrée avec un mot de passe vide : la release passe, avec un rappel.
            Write-Host "    ATTENTION : la clé de signature n'a pas de mot de passe ($keyHelp)" -ForegroundColor Yellow
            $keyPassword = ''
        }
        1 { }
        default { Fail "clé de signature illisible : $keyPath" }
    }
}
if ($null -eq $keyPassword) {
    if ($env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
        $keyPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
        $origin = 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD'
    } else {
        $keyPassword = Read-StoredSecret $credentialTarget
        $origin = "Gestionnaire d'identifiants ($credentialTarget)"
    }
    if ($null -eq $keyPassword) {
        $origin = 'saisie'
        try { $secure = Read-Host "    Mot de passe de la clé de signature" -AsSecureString }
        catch { Fail "mot de passe de la clé absent (session non interactive) : définir TAURI_SIGNING_PRIVATE_KEY_PASSWORD ($keyHelp)" }
        $keyPassword = [System.Net.NetworkCredential]::new('', $secure).Password
    }
    if ($python) {
        if (-not $keyPassword) { Fail "mot de passe de la clé vide ($origin)" }
        if ((Test-KeyPassword $keyPassword) -ne 0) { Fail "mot de passe de la clé incorrect ($origin)" }
        Write-Host "    mot de passe vérifié ($origin)"
    } elseif (-not $keyPassword) {
        # Sans python, rien ne distingue une clé sans mot de passe : Tauri dira s'il est faux.
        Write-Host "    mot de passe vide : clé supposée sans mot de passe ($keyHelp)" -ForegroundColor Yellow
    }
}

Step "build de Seaglass $version"
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
# Vide = variable absente sous PowerShell 5.1 : Tauri suppose alors un mot de passe vide.
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $keyPassword
try {
    pnpm tauri build --config (Join-Path $root 'src-tauri\tauri.release.conf.json')
    $buildExit = $LASTEXITCODE
} finally {
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
    $keyPassword = $null
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
Copy-Item $setup $out -Force

if (-not $Notes) {
    # Pas de `git describe` : sans tag, il écrit sur stderr, ce que PowerShell 5.1 traite
    # comme une erreur fatale avec $ErrorActionPreference = 'Stop'.
    $previous = git tag --list 'v*' --sort=-v:refname | Select-Object -First 1
    $range = if ($previous) { "$previous..HEAD" } else { 'HEAD' }
    $Notes = (git log $range --no-merges --pretty=format:'- %s' -n 30) -join "`n"
}
$manifest = [ordered]@{
    version   = $version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = (Get-Content "$setup.sig" -Raw).Trim()
            url       = "$baseUrl/$version/$setupName"
        }
    }
}
$manifestPath = Join-Path $root 'release\latest.json'
# UTF-8 sans BOM : l'updater lit du JSON strict.
[System.IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))

$size = [math]::Round((Get-Item $setup).Length / 1MB, 1)

# Page de téléchargement : l'adresse du dossier ($baseUrl/) mène toujours à la dernière version.
Step "page de téléchargement"
function Html($text) { [System.Net.WebUtility]::HtmlEncode($text) }
$fr = [System.Globalization.CultureInfo]::GetCultureInfo('fr-FR')
$noteItems = ($Notes -split "`r?`n" | Where-Object { $_.Trim() } | ForEach-Object {
    '      <li>' + (Html ($_ -replace '^\s*-\s*', '')) + '</li>'
}) -join "`n"
$fields = [ordered]@{
    '{{VERSION}}' = Html $version
    '{{DATE}}'    = Html (Get-Date).ToString('d MMMM yyyy', $fr)
    '{{SIZE}}'    = Html $size.ToString($fr)
    '{{URL}}'     = Html "$baseUrl/$version/$setupName"
    '{{NOTES}}'   = $noteItems
}
$page = Get-Content (Join-Path $PSScriptRoot 'download-page.html') -Raw -Encoding UTF8
foreach ($field in $fields.GetEnumerator()) { $page = $page.Replace($field.Key, $field.Value) }
$pagePath = Join-Path $root 'release\index.html'
[System.IO.File]::WriteAllText($pagePath, $page, [System.Text.UTF8Encoding]::new($false))

Write-Host "    installeur : release\$version\$setupName ($size Mo)"
Write-Host "    manifeste  : release\latest.json -> $baseUrl/latest.json"
Write-Host "    page       : release\index.html -> $baseUrl/"

# --- 5. Publication ---------------------------------------------------------------------
if (-not $Publish) {
    Write-Host "Build terminé, rien n'a été publié (ajouter -Publish)." -ForegroundColor Green
    exit 0
}

Step "publication vers $env:SEAGLASS_RELEASE_DEST"
$dest = $env:SEAGLASS_RELEASE_DEST
# Le manifeste part en dernier : l'updater ne voit la version qu'une fois l'installeur en place.
# Cible scp : « hôte:… » ou « utilisateur@hôte:… » (au moins 2 caractères avant « : »,
# pour ne pas confondre avec une lettre de lecteur comme « D: »).
if ($dest -match '^[^\\/:]{2,}:') {
    $sshHost, $remotePath = $dest -split ':', 2
    scp -r $out "$dest/"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (installeur)" }
    # scp reprend des droits Windows trop larges (dossier en écriture pour tous) : on les
    # remet à 755 / 644 avant de rendre la version visible.
    ssh $sshHost "chmod 755 '$remotePath/$version' && chmod 644 '$remotePath/$version'/*"
    if ($LASTEXITCODE -ne 0) { Fail "échec du réglage des droits sur le serveur" }
    scp $pagePath "$dest/index.html"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (page de téléchargement)" }
    scp $manifestPath "$dest/latest.json"
    if ($LASTEXITCODE -ne 0) { Fail "échec de scp (manifeste)" }
    ssh $sshHost "chmod 644 '$remotePath/index.html' '$remotePath/latest.json'"
} else {
    New-Item -ItemType Directory -Force $dest | Out-Null
    Copy-Item $out (Join-Path $dest $version) -Recurse -Force
    Copy-Item $pagePath (Join-Path $dest 'index.html') -Force
    Copy-Item $manifestPath (Join-Path $dest 'latest.json') -Force
}

Step "tag $tag"
git tag -a $tag -m "Seaglass $version"
git push origin $tag
if ($LASTEXITCODE -ne 0) { Fail "échec du push du tag" }
Write-Host "Seaglass $version publiée." -ForegroundColor Green
