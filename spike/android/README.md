# Spike Android (#24)

App jetable d'un seul écran : vérifier que **Media3 (ExoPlayer)** lit les MPD de TIDAL (FLAC en fMP4, `SegmentTemplate` + `SegmentTimeline`, segments signés) et noter le format réellement décodé. Elle ne parle pas à TIDAL : le PC lui sert le MPD, comme à mpv dans le spike de phase 0.

Rien n'est écrit sur disque (principe 1) : le MPD reste en mémoire sur le PC, Media3 lit les segments en streaming.

## Prérequis

- Android Studio (SDK Android 35 ; le NDK n'est pas nécessaire pour ce spike).
- Téléphone en **débogage USB** (Options pour les développeurs), relié en USB, `adb devices` le liste.
- Le spike Python connecté en PKCE : `python spike\play.py login --pkce`.

## Construire et installer

Ouvrir `spike/android/` dans Android Studio (il génère le wrapper Gradle), puis Run sur le téléphone. En ligne de commande, une fois le wrapper généré :

```powershell
cd spike\android; .\gradlew.bat installDebug
```

## Lancer un titre

```powershell
python spike\play.py serve <id> --quality lossless   # ou hires, high, low
adb reverse tcp:8765 tcp:8765
adb shell am start -a android.intent.action.VIEW -n ch.bebanana.seaglass.spike/.MainActivity -d http://127.0.0.1:8765/manifest.mpd
```

`serve` affiche les deux commandes `adb`. `adb reverse` rend le `127.0.0.1:8765` du PC joignable au même endroit sur le téléphone : rien n'est ouvert sur le réseau local.

Extras de `am start` :
- `--ez float true` : sortie PCM flottante (sinon Android décode le FLAC 24 bits en 16 bits).
- `--ez append true` : ajoute le titre à la file au lieu de remplacer (test de transition). Le second titre a besoin de son propre port : `serve <id2> --port 8766`, `adb reverse tcp:8766 tcp:8766`, URL en `:8766`.

## Lire les résultats

L'écran et `adb logcat -s SeaglassSpike` affichent : format d'entrée (`audio/flac`, fréquence, débit), décodeur (`c2.android.flac.decoder`…), configuration de l'`AudioTrack` (encodage, fréquence, offload), sorties audio, erreurs. Les signatures d'URL sont masquées.

## À vérifier

| Cas | Attendu |
|---|---|
| `lossless` (FLAC 16/44,1) | lecture, `audio/flac`, AudioTrack `s16` 44100 |
| `hires` (24/96 ou plus) | lecture ; `float` en sortie avec `--ez float true` |
| `high` (AAC-LC 320) et `low` (HE-AAC 96) | lecture |
| Saut dans la barre | reprise rapide, sans erreur |
| `append` d'un second titre | transition sans coupure audible |
| Écran éteint | sans service de lecture, la lecture peut s'arrêter : normal pour le spike |
