# Manifeste de l'updater du bureau (release/latest.json), écrit par release.ps1 et
# release-linux.ps1 (#314) : une seule fonction, pour que l'un n'efface pas l'autre.
# À charger après build-stamp.ps1 ; utilise $root et Fail du script.
#
# Le manifeste ne décrit que la version donnée, avec une entrée par plateforme dont le
# build est de cette version, de ce commit, et dans le même état (propre ou non) que
# l'arbre : jamais une entrée d'une version plus ancienne, que l'updater installerait.
#   windows-x86_64 : release/<version>/Seaglass_<version>_x64-setup.exe(.sig)
#   linux-x86_64   : release/<version>/Seaglass_<version>_amd64.AppImage(.sig)

$UpdaterPlatforms = [ordered]@{
    'windows-x86_64' = @{ stamp = 'windows'; file = { param($v) "Seaglass_${v}_x64-setup.exe" } }
    'linux-x86_64'   = @{ stamp = 'linux'; file = { param($v) "Seaglass_${v}_amd64.AppImage" } }
}

# Vrai si le dernier build de la plateforme est de cette version et de cet état de l'arbre.
function Test-FreshBuild([string]$platform, [string]$version) {
    $path = Get-BuildStampPath $platform
    if (-not (Test-Path $path)) { return $false }
    $stamp = Get-Content $path -Raw -Encoding UTF8 | ConvertFrom-Json
    $stamp.version -eq $version -and
        $stamp.commit -eq (git rev-parse HEAD) -and
        [bool]$stamp.dirty -eq [bool](git status --porcelain)
}

# Écrit release/latest.json ; renvoie les plateformes annoncées.
function Write-UpdaterManifest([string]$version, [string]$notes, [string]$baseUrl) {
    $platforms = [ordered]@{}
    foreach ($entry in $UpdaterPlatforms.GetEnumerator()) {
        if (-not (Test-FreshBuild $entry.Value.stamp $version)) { continue }
        $name = & $entry.Value.file $version
        $sig = Join-Path $root "release\$version\$name.sig"
        if (-not (Test-Path $sig)) { Fail "signature absente pour $($entry.Key) : $sig" }
        $platforms[$entry.Key] = [ordered]@{
            signature = (Get-Content $sig -Raw).Trim()
            url       = "$baseUrl/$version/$name"
        }
    }
    if ($platforms.Count -eq 0) { Fail "aucun build de la $version sur ce commit pour le manifeste de l'updater" }
    $manifest = [ordered]@{
        version   = $version
        notes     = $notes
        pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
        platforms = $platforms
    }
    $path = Join-Path $root 'release\latest.json'
    # UTF-8 sans BOM : l'updater lit du JSON strict.
    [System.IO.File]::WriteAllText($path, ($manifest | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))
    @($platforms.Keys)
}

# Avant d'envoyer latest.json : chaque fichier annoncé, sauf ceux que l'appelant vient de
# publier ($except), doit déjà être en ligne. Sinon l'updater d'une plateforme irait
# chercher un fichier absent (release Windows seule après un build Linux du même commit).
# Destination locale (essais) : le fichier y est cherché ; sinon, requête HEAD sur l'URL.
function Assert-ManifestOnline([string]$manifestPath, [string]$dest, [string[]]$except) {
    $manifest = Get-Content $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    # Même règle que la publication : « hôte:… » (2 caractères au moins, pas « D: »).
    $remote = $dest -match '^[^\\/:]{2,}:'
    foreach ($platform in $manifest.platforms.PSObject.Properties) {
        if ($except -contains $platform.Name) { continue }
        $url = $platform.Value.url
        if ($remote) {
            try {
                $response = Invoke-WebRequest -Uri $url -Method Head -UseBasicParsing -TimeoutSec 30
                $ok = $response.StatusCode -eq 200
            } catch { $ok = $false }
        } else {
            $ok = Test-Path (Join-Path $dest "$($manifest.version)\$(Split-Path $url -Leaf)")
        }
        if (-not $ok) {
            Fail ("latest.json annonce $($platform.Name), mais $url n'est pas en ligne :`n" +
                "    publier cette plateforme d'abord (release-all.ps1 le fait dans l'ordre)")
        }
        Write-Host "    $($platform.Name) déjà en ligne"
    }
}
