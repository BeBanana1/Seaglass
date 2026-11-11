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
| Connexion | fenêtre WebView2, redirection interceptée | navigateur du système, adresse de retour collée dans l'app (voir plus bas) |
| Contrôles média | SMTC | rien pour l'instant (MPRIS à faire, issue séparée) |
| Jetons | Gestionnaire d'identifiants (`windows-native`) | Secret Service (`sync-secret-service` + `crypto-rust`, dépendance propre à la cible Linux dans `Cargo.toml`) : demande un trousseau (GNOME Keyring, KWallet). Sans backend, `keyring` 3 se rabat sur un magasin **en mémoire** et la session est perdue à la fermeture : c'était le cas avant (#17). Sans service disponible, la connexion échoue avec une erreur au lieu de perdre la session en silence. `secret-service` et `zbus` 4 figurent dans `Cargo.lock` par une feature optionnelle de `keyring`, sans être compilés |

Bit-perfect : PipeWire suit la fréquence du flux si `default.clock.allowed-rates` contient
44,1 et 48 kHz (et leurs multiples) dans sa configuration ; sinon il rééchantillonne. ALSA
direct (`alsa/hw:…`) serait l'équivalent du mode exclusif : à étudier.

## Connexion par le navigateur du système

TIDAL sert une vérification anti-robot à WebKitGTK et, après quelques connexions, le
**bloque** (« You have been blocked », vu le 2026-11-11 dans WSL). Sous Linux, la connexion
passe donc par le navigateur de l'utilisateur, comme le flux PKCE de `tidalapi` :

1. `browser_login_start` crée le défi PKCE, le garde dans `commands::auth::BROWSER_LOGIN`
   (un seul à la fois), ouvre `login.tidal.com/authorize` par `tauri-plugin-opener` et
   renvoie l'adresse. Si aucun navigateur ne s'ouvre (WSL n'en a pas, `xdg-open` échoue),
   l'échec est seulement journalisé : l'écran affiche l'adresse avec un bouton Copier, à
   coller dans n'importe quel navigateur, celui de Windows compris. Elle ne contient que le
   défi, pas le vérificateur.
2. Après la connexion, TIDAL renvoie sur `https://tidal.com/android/login/auth?code=…`, une
   page d'erreur dans un navigateur de bureau. L'utilisateur colle son adresse dans l'app.
3. `browser_login_finish` n'accepte que cette adresse (`auth::redirect_code`, 4 Ko au plus,
   testé). Le défi n'est retiré qu'une fois la session ouverte : sinon un échec de
   l'échange ou du trousseau, suivi d'un nouvel essai, n'affichait plus que « ouvrez d'abord
   la page de connexion » et masquait la vraie erreur (vu dans WSL).

Le choix vient de Rust (`login_in_browser`, vrai sous Linux) ; l'écran est
`views/Login.svelte`. Aperçu : `?loggedout&browserLogin=1`.

## Construire et tester dans WSL

WSLg fournit l'affichage (Wayland/X) et un serveur PulseAudio (`PULSE_SERVER`) qui sort sur
le périphérique par défaut de Windows. Rien n'y est bit-perfect (chaîne RDP rééchantillonnée) :
WSL sert à vérifier que l'app se lance, se connecte et lit, pas la qualité.

Paquets (Ubuntu 24.04, en root : `wsl -u root`) :

```bash
apt-get install -y pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev libxdo-dev file patchelf mpv gnome-keyring libsecret-tools
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
- Connexion dans WebKitGTK (binaire nu, puis AppImage) : TIDAL sert une vérification
  anti-robot ; une fois l'épreuve résolue par l'utilisateur, la redirection est bien
  interceptée et la session ouverte. Après quelques connexions, TIDAL a bloqué WebKitGTK :
  d'où la connexion par le navigateur du système.

- Connexion depuis l'AppImage avec GStreamer embarqué : épreuve affichée, session ouverte.
  Sans backend Linux pour `keyring`, la session était perdue à la fermeture.

WSL a un bus de session D-Bus (systemd utilisateur) : `gnome-keyring` y est activé à la
demande, mais **sans trousseau par défaut** (l'alias `default` pointe vers un `login`
inexistant), et `keyring` ne le crée pas : limite documentée dans
`keyring/src/secret_service.rs` (keyring-rs#133). Le créer une fois avec `secret-tool`, qui
ouvre la fenêtre de GNOME pour choisir son mot de passe, puis effacer l'entrée de test :

```bash
secret-tool store --label=test seaglass-test x
secret-tool clear seaglass-test x
```

Un vrai bureau (GNOME, KDE) a ce trousseau dès l'ouverture de session.

- Connexion par le navigateur du système (adresse copiée dans le navigateur de Windows,
  page d'erreur finale de TIDAL collée dans l'app) : session ouverte, enregistrée dans
  GNOME Keyring une fois le trousseau créé. Lecture TIDAL par mpv sur la sortie `pulse`.

- Session gardée après un redémarrage de l'app, enchaînement sans blanc entre les titres
  d'un album, file et titre en cours repris en pause au redémarrage.

Pas vérifié : zone de notification (WSLg n'en a pas de représentative).
