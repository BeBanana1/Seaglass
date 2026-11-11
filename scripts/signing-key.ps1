# Mot de passe de la clé de signature de l'updater, partagé par release.ps1 (installeur
# Windows) et release-linux.ps1 (AppImage, #314) : les deux sont signés avec la même clé.
# À charger avec « . (Join-Path $PSScriptRoot 'signing-key.ps1') » ; utilise Fail du script.

# Mot de passe de la clé (#87) : TAURI_SIGNING_PRIVATE_KEY_PASSWORD (session), sinon entrée
# générique « seaglass-signing-key » du Gestionnaire d'identifiants, sinon saisi au clavier.
# Il ne passe que par l'environnement du build, jamais par un fichier, et il est vérifié
# avant le build (scripts/protect-signing-key.py --check, mot de passe sur l'entrée standard).
# Renvoie '' pour une clé sans mot de passe (avec un rappel).
function Get-SigningKeyPassword([string]$keyPath) {
    $credentialTarget = 'seaglass-signing-key'
    $keyHelp = "procédure : issue #87 / docs/release.md, « Signature »"

    . (Join-Path $PSScriptRoot 'credentials.ps1')

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
    $keyPassword
}
