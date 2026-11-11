# Linux (#17)

État : l'app se compile et lit sous Linux, sans paquet publié ni mises à jour. mpv vient du
système (`apt install mpv`), il n'est pas embarqué.

## Ce qui diffère du bureau Windows

| Domaine | Windows | Linux |
|---|---|---|
| Sortie audio | `--ao=wasapi` | `--ao=pipewire,pulse,alsa` : mpv prend le premier qui s'ouvre (`mpv::device::AO`) |
| Liste des périphériques | entrées `wasapi/` | entrées du premier pilote présent parmi `pipewire/`, `pulse/`, `alsa/` (`device::pick`) : PipeWire réapparaît sinon par sa couche PulseAudio et par ALSA |
| Nom du périphérique par défaut | `IMMDeviceEnumerator` | inconnu (`system_default_id` renvoie `None`) : la chaîne audio affiche « défaut du système » |
| Mode exclusif (#245) | `--audio-exclusive` | non proposé : ni PipeWire ni PulseAudio ne l'offrent par mpv, l'interrupteur est masqué (`exclusive_available`) |
| mpv | embarqué ; le `PATH` seulement en debug (#261) | celui du système (`PATH`, aussi en release : `execvp` ne cherche pas dans le dossier courant) |
| Arrêt de mpv sur plantage | Job Object (`mpv/job.rs`) | `prctl(PR_SET_PDEATHSIG, SIGKILL)` (`process::die_with_parent`) |
| IPC mpv | named pipe | socket Unix dans le dossier temporaire |
| Contrôles média | SMTC | rien pour l'instant (MPRIS à faire, issue séparée) |
| Jetons | Gestionnaire d'identifiants | Secret Service (`keyring`) : demande un trousseau (GNOME Keyring, KWallet) |

Bit-perfect : PipeWire suit la fréquence du flux si `default.clock.allowed-rates` contient
44,1 et 48 kHz (et leurs multiples) dans sa configuration ; sinon il rééchantillonne. ALSA
direct (`alsa/hw:…`) serait l'équivalent du mode exclusif : à étudier.

## Construire et tester dans WSL

WSLg fournit l'affichage (Wayland/X) et un serveur PulseAudio (`PULSE_SERVER`) qui sort sur
le périphérique par défaut de Windows. Rien n'y est bit-perfect (chaîne RDP rééchantillonnée) :
WSL sert à vérifier que l'app se lance, se connecte et lit, pas la qualité.

Paquets (Ubuntu 24.04, en root : `wsl -u root`) :

```bash
apt-get install -y pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev libxdo-dev file patchelf mpv
```

Node 20+ (Vite 8) n'est pas dans les dépôts d'Ubuntu 24.04 : le frontend est construit sous
Windows (`pnpm build`), puis l'arbre (avec `dist/` et `.env`) est copié dans le home de WSL
(compiler sur `/mnt/g` est très lent). `tauri-cli` vient de crates.io
(`cargo install tauri-cli --version "^2" --locked`). `beforeBuildCommand` est neutralisé, et
les artefacts de mise à jour aussi (ils exigent la clé de signature) :

```bash
cd ~/seaglass/src-tauri && cargo tauri build --bundles appimage --config '{"build":{"beforeBuildCommand":""},"bundle":{"createUpdaterArtifacts":false}}'
```

**GStreamer dans l'AppImage** (`bundle.linux.appimage.bundleMediaFramework`) : sans lui,
l'AppImage embarque `libgstreamer` mais aucun plugin, et ne trouve pas ceux du système
(`appsink`, `appsrc`, `autoaudiosink` « not found »). La page de vérification anti-robot
de TIDAL, servie à WebKitGTK à la connexion, n'affichait alors pas son épreuve et la
connexion était impossible. Le binaire nu (`target/release/seaglass`), qui prend le
GStreamer du système, l'affichait. Avec le plugin `gstreamer` de linuxdeploy (il demande
`patchelf`), l'AppImage passe de 83 à 97 Mo.

L'AppImage se lance avec `--appimage-extract-and-run` (pas de FUSE dans WSL). La
session `wsl.exe` qui l'a lancée doit rester ouverte : l'app s'arrête avec elle.

## Vérifié (2026-11-11, WSL Ubuntu 24.04, mpv 0.37)

- AppImage 0.5.5 construite (83 Mo) ; l'app démarre, mpv du système lancé comme enfant.
- `seaglass.conf` : les options absentes de mpv 0.37 ou propres à Windows
  (`media-controls`, `wasapi-exclusive-buffer`) ne font qu'un avertissement, mpv démarre.
- `audio-device-list` dans WSLg : `pulse/RDPSink`, `alsa`, `jack`, `sdl` → `device::pick`
  garde `pulse/RDPSink`. Avec `--ao=pipewire,pulse,alsa`, mpv essaie `pipewire` puis ouvre
  `pulse` (un `audio-device` préfixé `pulse/` ne fait pas sauter `pipewire`).
- `kill -9` de l'app : mpv disparaît aussitôt (`PR_SET_PDEATHSIG`).
- Connexion (binaire nu) : TIDAL sert une vérification anti-robot à WebKitGTK ; une fois
  l'épreuve résolue par l'utilisateur, la redirection est bien interceptée par
  `on_navigation` et la session ouverte.

À vérifier : connexion depuis l'AppImage avec GStreamer embarqué, jetons gardés après un
redémarrage (Secret Service, absent de WSL sans `gnome-keyring`), lecture TIDAL, gapless,
reprise de la file, zone de notification.
