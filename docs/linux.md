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

## Release (#313)

L'AppImage publiée se construit sous **Ubuntu 22.04**, dans une distribution WSL dédiée
(`Ubuntu-22.04`), et pas dans celle des essais (24.04). Une AppImage exige la glibc de la
machine qui l'a construite : sous 24.04, le binaire demandait `GLIBC_2.39` et ne démarrait
ni sur Debian 12 (2.36), ni sur Ubuntu 22.04 (2.35), ni sur Mint 21. La 22.04 est la base
la plus ancienne qui a WebKitGTK 4.1 (exigé par Tauri 2) : glibc 2.35, soit Ubuntu 22.04,
Debian 12, Mint 21, Fedora 36 et plus récentes (annoncé sur la page de téléchargement).

- **Préparer la distribution** (une fois, relançable) : `wsl --install -d Ubuntu-22.04
  --no-launch`, un utilisateur créé en root (`useradd -m`, `[user] default=` dans
  `/etc/wsl.conf`), puis `scripts/linux-build-setup.sh --system` en root (paquets, plugins
  GStreamer embarqués par `bundleMediaFramework`, mpv et playerctl pour les essais) et
  `scripts/linux-build-setup.sh` en utilisateur (rustup, `tauri-cli`).
- **Build** : `powershell -File scripts/release-linux.ps1` (appelé par `release-all.ps1`,
  qui accepte `-SkipLinux`). `pnpm build` sous Windows, puis `scripts/linux-build.sh` dans
  la distribution : copie de l'arbre dans `~/seaglass-release` (`target/` gardé),
  `cargo tauri build --bundles appimage`, signée pour l'updater (#314, clé par `WSLENV`), puis
  **contrôle de la glibc** : le plus haut `GLIBC_x.y` exigé par le binaire et par les `.so`
  de l'`AppDir` doit être ≤ `SEAGLASS_LINUX_GLIBC_MAX` (2.35 par défaut), sinon le build
  échoue. Sorties : `release/<version>/Seaglass_<version>_amd64.AppImage` et son `.sig`,
  `release/linux.json` (version, URL, SHA-256, taille, glibc, notes) et le repère
  `release/.build/linux.json`. `-PublishOnly`, publication (AppImage puis manifeste), tag
  et release Gitea comme pour Android.
- **Page de téléchargement** : carte Linux remplie depuis `linux.json` (même code que la
  carte Android), avec la mention « mpv requis ».
- Premier build (2026-11-11, 0.5.7) : compilation complète en 2 min, AppImage de 94 Mo,
  glibc exigée 2.35, 106 plugins GStreamer embarqués. Lancée sous Ubuntu 24.04 : reprise
  de la file et lecture HiRes 24/96, MPRIS publié. Lancée sous 22.04 : l'app et mpv
  démarrent ; la distribution de build n'a pas de bus de session (sa session systemd
  utilisateur ne démarre pas), MPRIS y est donc absent, avec un simple avertissement.
- **mpv 0.34** (Ubuntu 22.04, la plus vieille version visée ; Debian 12 a la 0.35) :
  `loadfile` en arguments nommés (#316) accepté, avec `start` et `af` appliqués ;
  `seaglass.conf` ne donne que les deux avertissements déjà connus (`media-controls`,
  `wasapi-exclusive-buffer`).

## Mises à jour (#314)

Fonctionnement et manifeste : `release.md` (puces « Mises à jour » et « Manifeste de
l'updater »). Essai de bout en bout le 2026-11-11 (0.5.8), avec une **clé de test jetable**
et un manifeste servi en `http://127.0.0.1:8765` (`dangerousInsecureTransportProtocol` dans
la config du build d'essai seulement), deux AppImage construites sous 22.04 (0.5.8 et une
fausse 0.5.9 par `"version"` dans `--config`), lancées sous 24.04 en **vraie AppImage**
(FUSE : `libfuse2t64`, `/dev/fuse` existe dans WSL2) :

- AppImage dans `~/Apps` : manifeste lu, 0.5.9 téléchargée, signature vérifiée,
  proposition « Mise à jour prête » ; « Installer et redémarrer » : fichier remplacé par la
  0.5.9 (empreinte), app relancée par `request_restart`, la nouvelle instance ne propose
  plus rien.
- AppImage dans un dossier `555` : message « impossible de remplacer l'AppImage : le
  dossier … n'est pas modifiable … », fichier intact.
- `--appimage-extract-and-run` : Tauri valide `APPIMAGE`, l'updater est actif et
  viserait le vrai fichier.
- Binaire nu (`--no-bundle`) : « mises à jour automatiques désactivées ».
- Pilotage de la fenêtre dans WSLg : `GDK_BACKEND=x11`, puis `xwininfo` (position absolue),
  `xdotool` (clic) et `import` (capture) ; `wmctrl` ne voit pas les fenêtres.

Pendant ces essais, la sortie `pulse` de WSLg s'est figée (mpv bloqué à 0 % avec ou sans
l'app) : la lecture n'a pas pu être revérifiée sur l'AppImage mise à jour. Remarque : mpv
hérite du `LD_LIBRARY_PATH` posé par l'AppImage et charge alors 95 bibliothèques de
celle-ci au lieu de celles du système ; sans effet constaté jusqu'ici.

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

- `loadfile` (#316) : la forme positionnelle `loadfile <url> <flags> -1 <options>` n'existe
  que depuis mpv 0.38 (argument `index` ajouté avant `options`). Le mpv 0.37 d'Ubuntu 24.04
  répondait `invalid parameter` à toute reprise à une position et à la normalisation. Le
  moteur envoie maintenant `loadfile` en **arguments nommés**
  (`{"name": "loadfile", "url", "flags", "options"}`), compris par 0.37 comme par 0.41 :
  `live_per_file_af` passe dans les deux cas, et la reprise de la file à 91 s lit bien.
- Contrôles média (MPRIS, #312) pilotés par `playerctl` : voir `systeme.md`. Pour essayer
  sans AppImage : `cargo tauri build --no-bundle` avec la même configuration, puis
  `target/release/seaglass` (même dossier de données, même trousseau).

Pas vérifié : zone de notification (WSLg n'en a pas de représentative), applet média et
touches média d'un vrai bureau.
