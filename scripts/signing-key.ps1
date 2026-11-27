# Password of the updater signing key, shared by release.ps1 (Windows installer) and
# release-linux.ps1 (AppImage, #314): both are signed with the same key.
# Load with ". (Join-Path $PSScriptRoot 'signing-key.ps1')"; uses the script's Fail.

# Password of the key (#87): TAURI_SIGNING_PRIVATE_KEY_PASSWORD (session), otherwise the
# generic entry "seaglass-signing-key" of the Credential Manager, otherwise typed at the
# keyboard. It only goes through the build environment, never through a file, and it is
# checked before the build (scripts/protect-signing-key.py --check, password on standard
# input). Returns '' for a key without a password (with a reminder).
function Get-SigningKeyPassword([string]$keyPath) {
    $credentialTarget = 'seaglass-signing-key'
    $keyHelp = "procedure: issue #87 / docs/release.md, `"Signing`""

    . (Join-Path $PSScriptRoot 'credentials.ps1')

    # Exit code of --check: 0 = the password opens the key, 1 = it does not, 2 = unreadable key.
    $python = Get-Command python -ErrorAction SilentlyContinue
    function Test-KeyPassword([string]$password) {
        # UTF-8 for non-ASCII passwords (the BOM added by PowerShell 5.1 is stripped).
        $OutputEncoding = [System.Text.UTF8Encoding]::new($false)
        $password | & $python.Source -I (Join-Path $PSScriptRoot 'protect-signing-key.py') --check $keyPath | Out-Null
        $LASTEXITCODE
    }

    $keyPassword = $null
    if (-not $python) {
        Write-Host "    python not found: password not checked before the build" -ForegroundColor Yellow
    } else {
        switch (Test-KeyPassword '') {
            0 {
                # Key still encrypted with an empty password: the release goes ahead, with a reminder.
                Write-Host "    WARNING: the signing key has no password ($keyHelp)" -ForegroundColor Yellow
                $keyPassword = ''
            }
            1 { }
            default { Fail "signing key unreadable: $keyPath" }
        }
    }
    if ($null -eq $keyPassword) {
        if ($env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
            $keyPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
            $origin = 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD'
        } else {
            $keyPassword = Read-StoredSecret $credentialTarget
            $origin = "Credential Manager ($credentialTarget)"
        }
        if ($null -eq $keyPassword) {
            $origin = 'typed'
            try { $secure = Read-Host "    Signing key password" -AsSecureString }
            catch { Fail "key password missing (non-interactive session): set TAURI_SIGNING_PRIVATE_KEY_PASSWORD ($keyHelp)" }
            $keyPassword = [System.Net.NetworkCredential]::new('', $secure).Password
        }
        if ($python) {
            if (-not $keyPassword) { Fail "key password empty ($origin)" }
            if ((Test-KeyPassword $keyPassword) -ne 0) { Fail "key password incorrect ($origin)" }
            Write-Host "    password checked ($origin)"
        } elseif (-not $keyPassword) {
            # Without python, nothing tells a passwordless key apart: Tauri will say if it is wrong.
            Write-Host "    empty password: key assumed to have no password ($keyHelp)" -ForegroundColor Yellow
        }
    }
    $keyPassword
}
