# Seaglass

Client desktop TIDAL non officiel, avec une UI entièrement personnalisée et une lecture lossless / bit-perfect déléguée à mpv.
Projet personnel, Windows en priorité.

> Statut : **v0.4.4 publiée** (2026-10-29) sur `https://updates.bebanana.ch/seaglass/` : installeur NSIS signé, mpv embarqué, mises à jour automatiques. Phase 4 livrée (SMTC, cinq thèmes) ; Discord RPC et Last.fm reportés à la demande de l'utilisateur. Restent en phase 5 : mode exclusif WASAPI, mixes de l'accueil, CI.

## Principes non négociables

1. **Streaming uniquement.** Aucun téléchargement, export ou mise en cache des flux audio sur disque. Aucun contournement de DRM : si un flux est protégé par Widevine, on ne le lit pas, on remonte une erreur claire.
2. **Rust est la source de vérité.** L'UI n'a aucune logique métier : elle affiche l'état reçu du backend et envoie des commandes. Elle ne parle jamais directement à TIDAL ni à mpv.
3. **Tout ce qui touche TIDAL vit dans `src-tauri/src/tidal/`.** C'est une API privée non documentée qui peut changer à tout moment : le reste du code ne dépend que des types de `tidal::models`, jamais des formats JSON bruts.
4. **Aucun secret dans le dépôt.** Client ID / secret dans `.env` (gitignoré) ou variables d'environnement. Tokens utilisateur dans le Gestionnaire d'identifiants Windows via la crate `keyring`. Ne jamais logger un token, même tronqué.
5. **Pas de nom ni de logo TIDAL** dans le nom de l'app, l'icône ou le titre de fenêtre. Mention « Client non officiel, non affilié à TIDAL » dans l'écran À propos et le README.

## Stack

| Couche | Choix |
|---|---|
| App | Tauri 2 |
| Backend | Rust stable, `tokio`, `serde`, `reqwest` (rustls), `thiserror` + `anyhow`, `tracing` |
| Frontend | Svelte 5 + TypeScript + Vite, `pnpm` |
| Audio | mpv piloté en JSON IPC (named pipe), sidecar Tauri à terme |
| Stockage | SQLite (`rusqlite`) pour la file, les réglages et le cache de métadonnées |
| Secrets | `keyring` (Credential Manager sous Windows) |
| Intégrations | SMTC Windows (`windows` crate), Discord RPC, Last.fm (plus tard) |
| Mises à jour | `tauri-plugin-updater` (installeur NSIS signé, manifeste `latest.json`) |

Demander avant d'ajouter une dépendance qui n'est pas dans cette liste.

## Arborescence cible

```
seaglass/
├─ CLAUDE.md
├─ .gitea/workflows/ci.yml   # CI (runner dédié, voir ci/)
├─ ci/                       # image des jobs CI et runner act_runner (Docker, serveur)
├─ spike/                    # phase 0 : scripts Python jetables
├─ src/                      # frontend Svelte
│  ├─ lib/ipc.ts             # wrappers typés des commandes et événements Tauri
│  ├─ lib/transport.ts       # invoke/listen réels, ou backend factice hors Tauri (dev)
│  ├─ lib/mock/              # backend factice + fixtures.json (gitignoré) pour prévisualiser l'UI
│  ├─ lib/stores/            # état miroir du backend (player, queue, library, ui, nav)
│  ├─ components/            # dont backdrops/ (fonds par thème)
│  ├─ themes/                # acidwave.css, surreal.css (surcharges par classe de composant)
│  └─ views/                 # Home, Search, Album, Artist, Playlist, Library
└─ src-tauri/
   ├─ mpv/seaglass.conf        # config mpv dédiée à l'app
   └─ src/
      ├─ lib.rs              # setup Tauri, lancement des tâches
      ├─ commands/           # auth.rs, player.rs, catalog.rs, settings.rs : valident l'entrée puis délèguent
      ├─ tidal/              # auth.rs, api.rs, catalog.rs, raw.rs (JSON bruts), models.rs, cache.rs, manifest.rs (DRM)
      ├─ player/             # actor.rs, queue.rs, state.rs
      ├─ mpv/                # process.rs, ipc.rs, events.rs, serve.rs (MPD servis en HTTP local)
      ├─ store/              # SQLite
      └─ integrations/       # mod.rs (NowPlaying, Observers), smtc.rs ; discord.rs plus tard
```

## Architecture du lecteur

- **Acteur unique.** Une seule tâche tokio possède `Player` (file + état). Tout passe par un `mpsc::Sender<Cmd>` : commandes UI, événements mpv, SMTC. Pas de `Mutex` sur l'état du lecteur.
- **File d'attente** (`player/queue.rs`) :
  - `context: Vec<TrackId>` (album / playlist d'origine), `order: Vec<usize>` (ordre de lecture, mélangé ou non), `pos`
  - `manual: VecDeque<TrackId>` prioritaire sur le contexte (« Lire ensuite » = `push_front`, « Ajouter à la file » = `push_back`)
  - `history: Vec<TrackId>` pour « Précédent », indépendant du shuffle
  - `repeat: Off | All | One`
  - Shuffle = Fisher-Yates sur les pistes restantes, piste courante en tête. Désactiver le shuffle restaure l'ordre d'origine sur la piste courante.
- **Fenêtre mpv de 2 entrées maximum** : piste en cours + suivante. Les URLs de flux sont signées et expirent, donc on ne charge jamais plus loin.
  - Préchargement de la suivante (`loadfile <url> append`) quand `duration - time_pos < 30 s`.
  - Sur `playlist-pos == 1` : avancer la file, `playlist-remove 0`, `next_loaded = false`.
  - **Toute modification de la file** (lire ensuite, shuffle, retrait, repeat) → `playlist-remove 1` si une suivante est chargée, puis `next_loaded = false`.
- **Repeat One** = `loop-file=inf` côté mpv, pas de rechargement.
- **Précédent** : si `time_pos > 3 s`, seek à 0, sinon dernière piste de `history` en `loadfile … replace`.
- **Erreur de lecture** (`end-file` reason `error`) : redemander une URL une fois, puis passer à la suivante et émettre un événement `toast`.
- **Vers l'UI** : événement `player-state` émis seulement si l'état a changé, avec `time_pos` limité à environ 4 Hz. L'UI interpole la barre de progression avec `requestAnimationFrame`. La liste de la file part dans un événement séparé `queue-state`, émis seulement quand la file change (sinon elle serait renvoyée 4 fois par seconde).
- **Persistance** : file et position sauvegardées dans SQLite (debounce de 2 s), restaurées en pause au démarrage.

## mpv

- Lancé par l'app avec `--no-config --include=<seaglass.conf>` pour **ne jamais** charger la config mpv vidéo personnelle de l'utilisateur.
- Options de base : `--idle=yes --no-video --input-ipc-server=\\.\pipe\seaglass-mpv-<pid de l'app> --ao=wasapi --gapless-audio=weak --prefetch-playlist=yes`. Le pid dans le nom du pipe empêche de se connecter à un mpv orphelin d'un lancement précédent.
- Tauri quitte sans exécuter les destructeurs : mpv est tué explicitement sur `RunEvent::Exit` (`mpv::process::ProcessGuard`). En cas de plantage, c'est le Job Object (voir plus bas) qui l'arrête.
- `media-controls=no` et `input-media-keys=no` dans `seaglass.conf` : sinon mpv publie sa propre session SMTC « mpv », qui capte les touches média (cause des issues #6 et #27). Seule la session de Seaglass doit exister.
- `audio-params` passe à `null` à chaque fin de fichier et n'est pas renvoyé si le titre suivant a le même format : un `null` est ignoré, la sortie n'est effacée que sur `idle-active` (#12, `PlayerState::set_output`).
- Mode bit-perfect (réglage utilisateur) : `--audio-exclusive=yes`, volume mpv à 100, pas de `replaygain`, pas de `audio-samplerate` forcé, pas de filtres audio.
- Propriétés observées : `playlist-pos`, `time-pos`, `duration`, `pause`, `paused-for-cache`, `audio-params`, `current-ao`, `idle-active`. Événement : `end-file`.
- Un micro-blanc lors d'un changement de fréquence d'échantillonnage en mode exclusif est **normal** (réouverture du périphérique). Ce n'est pas un bug à corriger.
- Résolution de mpv (`mpv::process::executable`) : `SEAGLASS_MPV_PATH`, sinon `mpv.exe` à côté de `seaglass.exe` (installé par l'installeur), sinon celui du `PATH` (dev).
- mpv est rattaché à un Job Object Windows `KILL_ON_JOB_CLOSE` (`mpv/job.rs`) : il meurt avec l'app, même sur un plantage.

## Feuille de route

0. ~~**Spike**~~ ✅ : script Python `spike/play.py` avec `tidalapi` : connexion, récupération du manifeste d'un titre, lecture dans mpv. Flux FLAC sans DRM confirmés avec le client PKCE.
1. ~~**Squelette**~~ ✅ : Tauri + Svelte, module `mpv/` (process + IPC + événements), commandes `login` (PKCE), `search`, `play`, `toggle_pause`, `seek`. Acteur lecteur minimal (un titre à la fois).
2. ~~**File d'attente**~~ ✅ : `queue.rs` (testé), gapless, retry sur erreur, repeat One via `loop-file`, persistance SQLite.
3. ~~**Vues**~~ ✅ : accueil, recherche multi-types, album, artiste, playlist, bibliothèque (playlists, albums, artistes, titres).
4. ~~**Intégrations + UI**~~ ✅ : SMTC ; thèmes Verre de mer / Studio / Atelier / Acidwave / Surréalisme, vue « En lecture », chaîne audio, raccourcis clavier. Discord RPC et Last.fm reportés (pas avant que l'utilisateur le demande).
5. **(actuel)** Finitions : ~~installeur NSIS, mpv embarqué + Job Object, mises à jour signées, script de release~~ ✅ ; ~~hébergement des mises à jour~~ ✅ (v0.4.4 publiée) ; restent le mode exclusif WASAPI (réglage), les mixes de l'accueil et la CI.

## Résultats du spike (2026-10-29, tidalapi 0.8.11, abonnement HI_RES, pays CH)

Observé avec `spike/play.py probe` puis décodage réel dans mpv 0.41.

- **Le type de connexion détermine la qualité, pas l'abonnement.**
  - Client « appareil » (device code, `link.tidal.com`) : manifeste `application/vnd.tidal.bts`, **AAC 320 kbps maximum**, même en demandant `LOSSLESS` ou `HI_RES_LOSSLESS`. Inutilisable pour l'app.
  - Client PKCE (identifiant Android embarqué dans tidalapi, redirection vers `https://tidal.com/android/login/auth?code=…`) : manifeste `application/dash+xml` pour toutes les qualités. **C'est le flux de connexion à implémenter en phase 1** (la webview Tauri peut intercepter la redirection, ce qui évite le copier-coller).
- **Formats reçus (client PKCE)** :

  | Demandé | Reçu | Codec | Format |
  |---|---|---|---|
  | `LOW` | LOW | HE-AAC `mp4a.40.5` | ~96 kbps |
  | `HIGH` | HIGH | AAC-LC `mp4a.40.2` | ~320 kbps |
  | `LOSSLESS` | LOSSLESS | FLAC dans fMP4 | 16/44,1 |
  | `HI_RES_LOSSLESS` | HI_RES_LOSSLESS | FLAC dans fMP4 | 24/44,1 et 24/48 vus |

  Un titre sans version hi-res renvoie LOSSLESS 16/44,1 quand on demande HI_RES. Un titre Dolby Atmos renvoie du FLAC stéréo. Pas encore vu de 24/96 ni de 24/192.
- **Pas de DRM** : aucun élément `ContentProtection`, aucun `cenc:default_KID`/`pssh` dans le MPD, aucune boîte `pssh`/`tenc`/`sinf`/`enca` dans le segment d'initialisation. Attention : le MPD déclare **toujours** `xmlns:cenc`, même en clair, donc ce n'est pas un marqueur de DRM.
- **Métadonnées** : `track.audioQuality` vaut `LOSSLESS` même pour un titre hi-res, la disponibilité hi-res se lit dans `mediaMetadata.tags` (`HIRES_LOSSLESS`, `DOLBY_ATMOS`). Le format réel (`bitDepth`, `sampleRate`) n'est connu qu'avec `playbackinfopostpaywall`.
- **Lecture** : le MPD utilise `SegmentTemplate` + `SegmentTimeline` avec des URLs absolues signées. mpv le lit via le démuxeur DASH de ffmpeg s'il est servi en HTTP. Le spike le sert depuis la mémoire sur `127.0.0.1` (rien sur disque). Le backend Rust devra faire de même (petit serveur local) ou valider une autre approche. mpv sort en `s32` pour du 24 bits, c'est normal.
- À surveiller : un avertissement ffmpeg `dash: Error when loading first fragment of playlist` vu une fois sur un titre 24/48, sans effet sur la lecture.

## Implémentation (phase 1)

- **Identifiant client** : `TIDAL_CLIENT_ID` / `TIDAL_CLIENT_SECRET` (client PKCE) dans `.env`, injectés à la compilation par `build.rs`. Une variable d'environnement au lancement reste prioritaire.
- **Connexion** : fenêtre webview `login` (sans aucune capability) sur `login.tidal.com/authorize` ; `on_navigation` intercepte `https://tidal.com/android/login/auth?code=…` et bloque la navigation. Refresh token dans le Credential Manager (service `seaglass`, utilisateur `tidal-refresh-token`). L'access token (4 h) n'est jamais persisté, il est rafraîchi 60 s avant expiration ou sur 401.
- **Garde DRM** : avant chaque lecture, `tidal::manifest` rejette les MPD avec `ContentProtection` / `cenc:default_KID` / `cenc:pssh` / UUID Widevine, puis le segment d'initialisation (lu en mémoire) s'il contient `pssh`/`tenc`/`sinf`/`enca`.
- **Manifestes** : `mpv::serve` écoute sur `127.0.0.1:<port aléatoire>` et garde les 4 derniers MPD en mémoire ; mpv reçoit `http://127.0.0.1:<port>/<track>-<n>.mpd`.

## Implémentation (phase 2)

- **Acteur** découpé en `actor.rs` (boucle, commandes, émission), `playback.rs` (fenêtre mpv, gapless, erreurs) et `persist.rs`. Les résolutions réseau (métadonnées + flux) tournent hors de l'acteur et reviennent en `Cmd::Resolved` ; les compteurs `load_req` / `preload_req` écartent les réponses périmées.
- **Précédent** : l'historique garde l'origine de chaque titre. Revenir sur un titre du contexte replace le curseur (« Suivant » ramène au titre quitté) ; un titre courant issu de la file manuelle y est remis en tête.
- **Shuffle** : Fisher-Yates sur tous les autres titres du contexte, piste courante en tête.
- **Erreurs** : nouvelle URL une fois par titre, puis titre suivant avec toast ; arrêt après 3 échecs consécutifs.
- **Persistance** : `seaglass.db` dans `%APPDATA%\dev.seaglass.app\` (tables `kv` et `tracks`). Écriture au plus toutes les 2 s : position seule, ou file + métadonnées des titres référencés si la file a changé. Au démarrage, la file est restaurée en pause et le titre n'est chargé dans mpv qu'au premier clic sur Lecture, à la position sauvegardée.
- **Cache de métadonnées** en mémoire (`tidal::cache`), alimenté par la recherche ; il permet d'afficher la file sans requête.
- Vérifié en lecture réelle : préchargement à 25 s de la fin (`playlist-count=2`), bascule sans rechargement puis `playlist-count=1`, restauration à la bonne position avec mpv inactif.
- Vérifié en lecture réelle : `audio-codec-name=flac`, `current-ao=wasapi`, sortie `s16` 44,1 kHz pour un titre 16/44,1.

## Implémentation (phase 3)

- **Endpoints** (vérifiés, test `cargo test live_catalog -- --ignored --nocapture`) : `search?types=TRACKS,ALBUMS,ARTISTS,PLAYLISTS`, `albums/{id}` + `/tracks`, `artists/{id}` + `/toptracks` + `/albums` (`filter=EPSANDSINGLES` pour les singles), `playlists/{uuid}` + `/tracks`, `users/{id}/favorites/{tracks,albums,artists}` (`order=DATE&orderDirection=DESC`, éléments `{created, item}`), `users/{id}/playlistsAndFavoritePlaylists` (éléments `{type, playlist}`), v2 `home/feed/static?deviceType=BROWSER&platform=WEB&locale=fr_FR`.
- **Pagination** : avancer selon `totalNumberOfItems`, pas selon le nombre d'éléments reçus (les titres indisponibles sont filtrés : 97 reçus pour `limit=100`). Pages de 100, sauf `playlistsAndFavoritePlaylists` qui refuse plus de 50.
- **Accueil v2** : lignes `{type, title, items: [{type, data}]}`. Seuls `ALBUM`, `ARTIST` et `PLAYLIST` sont gardés ; `MIX`, `TRACK` (sans album) et `DEEP_LINK` sont ignorés pour l'instant. Avec `locale=fr_FR`, les titres de lignes sont en français.
- **Images** : `resources.tidal.com/images/<id avec / à la place des ->/<n>x<n>.jpg`. Pochettes 320 (listes) et 640 (en-tête d'album), photos d'artiste et playlists (`squareImage`) en 320.
- **`MediaItem`** est sérialisé avec l'étiquette `kind` : aucun type qu'il contient ne doit avoir de champ `kind` (d'où `AlbumSummary::album_type`, test `media_item_tag_is_unique`).
- **UI** : navigation par pile de routes (`lib/stores/nav.svelte.ts`) sans URL, bouton Retour dans l'en-tête. Lire un titre d'une liste met toute la liste en contexte ; « Aléatoire » passe par `play_tracks(…, shuffle: true)`, le backend choisit le titre de départ.

## Implémentation (phase 4)

- **SMTC** (`integrations/smtc.rs`) : `ISystemMediaTransportControlsInterop::GetForWindow(hwnd de la fenêtre main)` sur un thread dédié (MTA). L'acteur notifie `Observers` à chaque `player-state` émis ; le thread ne touche SMTC que si les métadonnées, le statut ou les boutons changent, et la timeline toutes les 5 s ou après un saut. Boutons → `Cmd::SetPaused(bool)`, `Next`, `Previous` (`try_send`, thread WinRT). Vérifié avec l'API `GlobalSystemMediaTransportControlsSessionManager` : session `seaglass.exe`, titre / artiste / album / vignette / timeline, et Play / Pause / Suivant / Précédent pilotent bien le lecteur.
- **État restauré** : `pause` de mpv est ignoré tant qu'aucun titre n'est chargé (sinon le `pause=false` initial efface l'état « en pause » restauré) ; `paused=false` est posé explicitement après un chargement.
- **Couleur d'album** : `vibrantColor` → `AlbumRef::color` / `AlbumSummary::color`, filtrée en `#rrggbb` (elle part dans le CSS). L'UI la pose en `--album` sur `<html>`.
- **Réglages UI** : `ui_settings` / `set_ui_settings`, JSON dans `kv` (`ui.settings`). Thème inconnu refusé à la désérialisation.
- **Thèmes** : `data-theme` sur `<html>`, jetons CSS dans `app.css`. Verre de mer : panneaux translucides sur la pochette floutée (`Ambient.svelte`) ; Studio : à plat, angles vifs, chaîne audio complète dans la barre ; Atelier : clair, titres serif. Les trois premiers ont leurs variantes dans les composants (`:global([data-theme=…])`) ; **Acidwave** et **Surréalisme** sont regroupés dans `src/themes/<thème>.css`, qui surchargent par nom de classe de composant (spécificité supérieure aux styles scopés) : renommer une classe de composant peut casser ces thèmes.
- **Fonds** (`components/backdrops/Backdrop.svelte`) : un par thème, derrière le shell et la vue « En lecture ». Acidwave : nappes holographiques, grille en perspective, trame, grain (`AcidField`), animés seulement si `data-playing="true"` sur `<html>`. Surréalisme : paysage SVG dessiné à la main (`SurrealScene`), teinté par `--album` avec des mélanges `oklch` (en `oklab`, bleu + pêche donnait du gris) ; les objets restent sur les bords, le centre est au contenu.
- **Chaîne audio** (`SignalPath.svelte`) : source → mpv → sortie, verdict « Fréquence native » / « Rééchantillonné » / « Profondeur réduite » / « Compressé ». En mode partagé, Windows peut encore mixer : c'est dit dans le détail du verdict, pas présenté comme bit-perfect.
- **Raccourcis** : Espace (lecture/pause), Ctrl+K ou `/` (recherche), Échap (fermer), Alt+← (retour).
- **Prévisualisation sans Tauri** : `pnpm dev` puis `http://127.0.0.1:1420/?theme=studio&route=album&nowplaying=1` (paramètres : `theme`, `route`, `tab`, `nowplaying`, `queue`, `paused`, `loggedout`). Données de `src/lib/mock/fixtures.json`, généré par `cargo test dump_ui_fixtures -- --ignored` (gitignoré : contient la bibliothèque du compte). La branche factice est éliminée du build de production (`import.meta.env.DEV`).
- **Piège Vite** : à deux reprises, après des éditions de fichiers par script, le CSS d'un composant est resté périmé alors que son JS était à jour (règles absentes du `?svelte&type=style`). Redémarrer `pnpm tauri dev` et vider `node_modules/.vite` règle le problème. Le panneau de navigateur intégré, quand il est masqué, gèle aussi les animations et le chargement différé des images : pour une capture, injecter `*{animation:none!important}`.

## Implémentation (phase 5 : installeur et mises à jour)

- **Installeur** : NSIS uniquement (`bundle.targets`), installation par utilisateur (`installMode: currentUser`, pas d'admin), dans `%LOCALAPPDATA%\Seaglass`. Icône : `src-tauri/icons/app-icon.svg`, déclinée avec `pnpm tauri icon src-tauri/icons/app-icon.svg` (supprimer ensuite `icons/android` et `icons/ios`). Licences livrées dans `licenses/` (GPL-3.0 de Seaglass, notice mpv).
- **mpv embarqué** : seulement au build de release, via `src-tauri/tauri.release.conf.json` (`externalBin: binaries/mpv`). Le script copie le mpv du `PATH` en `src-tauri/binaries/mpv-x86_64-pc-windows-msvc.exe` (gitignoré, 119 Mo). Dans la config de base, `externalBin` ferait échouer tout `cargo build` sans ce fichier. Le build actuel de mpv (zhongfly, x86-64-v3) exige un processeur AVX2.
- **Mises à jour** (`src-tauri/src/updater.rs`) : vérification 20 s après le démarrage puis toutes les 6 h ; téléchargement en arrière-plan, signature vérifiée par le plugin ; événement `update-ready` → `UpdatePrompt.svelte` (« Installer et redémarrer » = `install_update`, « Plus tard » = installation sur `RunEvent::Exit`, sans relance). `on_before_exit` arrête mpv avant que l'installeur ferme l'app. Rien n'est vérifié en debug sauf si `SEAGLASS_UPDATE_URL` est défini (aussi utile pour tester un autre manifeste).
- **Signature** : clé minisign dans `%USERPROFILE%\.tauri\seaglass.key` (sans mot de passe, hors du dépôt), clé publique dans `tauri.conf.json` (`plugins.updater.pubkey`). **Perdre la clé privée** = plus aucune mise à jour acceptée par les versions installées (réinstallation manuelle) : elle doit être sauvegardée.
- **Hébergement** (en service) : le Gitea exige une connexion partout, donc l'updater ne peut pas y lire de releases. Les mises à jour sont servies par le Caddy du serveur (Docker) : `updates.bebanana.ch` (CNAME vers `bebanana.internet-box.ch`) sert `/srv/updates` en lecture seule, `latest.json` en `Cache-Control: no-cache`. Le dossier est public, les installeurs étant signés.
- **Publication** : utilisateur `seaglass-deploy` (sans mot de passe ni sudo, clé SSH avec `restrict`), alias `seaglass-updates` dans `~/.ssh/config` du PC (IP locale du serveur), `SEAGLASS_RELEASE_DEST=seaglass-updates:/srv/updates/seaglass` (variable utilisateur Windows). Après `scp`, le script remet les droits à 755 / 644 : sinon le dossier de version arrive en écriture pour tous.
- **Release** : `pnpm release` (`scripts/release.ps1`) vérifie l'arbre git, `.env`, la clé et `VERSION`, embarque mpv, lance `pnpm tauri build --config src-tauri/tauri.release.conf.json`, puis écrit `release/latest.json` et `release/<version>/Seaglass_<version>_x64-setup.exe`. Avec `-Publish`, il copie vers `SEAGLASS_RELEASE_DEST` (dossier ou cible scp), le manifeste en dernier, puis pose et pousse le tag `v<version>`. Une release = un tag : pas de mise à jour à chaque push.

## Implémentation (phase 5 : CI)

- **Workflow** : `.gitea/workflows/ci.yml`, sur push vers `main` et sur chaque PR, sauf si seuls des `*.md` / `LICENSE` changent : `pnpm check`, `pnpm build`, `cargo fmt --check`, `clippy -D warnings`, `cargo test`. `pnpm build` passe avant Rust car `tauri::generate_context!` exige `dist/`. Ni `.env` ni `fixtures.json` ne sont nécessaires.
- **Runner dédié** (`ci/`, installation dans `ci/README.md`) : `act_runner` 0.6.1 en Docker sur le serveur, enregistré **au niveau du dépôt**, label `seaglass`. Il monte le socket Docker, donc il équivaut à root sur le serveur : ne jamais l'ouvrir à d'autres dépôts. Le runner partagé (firmware, KiCad) n'est plus utilisé.
- **Image des jobs** `seaglass-ci:<tag daté>` (`ci/Dockerfile`, base `node:24-bookworm`) : bibliothèques de Tauri, Rust stable + rustfmt + clippy, pnpm. Construite sur le serveur (`docker compose --profile build build`), sans registre (`force_pull: false`). Changer de tag = `.env` + label de `config.yaml`.
- **Cache** : les jobs rejoignent le réseau du runner (`container.network: seaglass-ci`) et le cache écoute sur un port fixe (8088). Sans ça, chaque action de cache attend 5 min un `ETIMEDOUT` (vu sur le runner partagé).
- **Limite** : sous Linux, le code `#[cfg(windows)]` (SMTC, Job Object, `CREATE_NO_WINDOW`) n'est ni compilé ni testé ; `pnpm release` le vérifie sous Windows. Du code partagé utilisé seulement par une intégration Windows doit tolérer le code mort hors Windows (`#[cfg_attr(not(windows), allow(dead_code))]`, voir `integrations/mod.rs` et `Cmd::SetPaused`), sinon clippy échoue en CI. L'IPC mpv passe par un socket Unix hors Windows (`mpv::ipc::open`).
- **Durée** : environ 7 min sur le runner partagé, sans cache (préparation ~4 min, clippy ~1 min, tests ~1 min 30). Sur le runner dédié, à mesurer.
- **Pas de release en CI** : la clé minisign reste sur le PC, `pnpm release` reste manuel.

## Commandes

PowerShell sous Windows.

```powershell
# Spike
python -m venv spike\.venv; spike\.venv\Scripts\Activate.ps1
pip install -r spike\requirements.txt
python spike\play.py login --pkce
python spike\play.py search "<requête>"
python spike\play.py probe <id>
python spike\play.py play <id> --exclusive

# App (à partir de la phase 1)
pnpm install
pnpm tauri dev
cd src-tauri; cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test
cargo test live_catalog -- --ignored --nocapture   # endpoints réels (session requise)
cargo test dump_ui_fixtures -- --ignored          # données de prévisualisation de l'UI
pnpm check
pnpm release                                      # installeur signé dans release/ (voir phase 5)
```

## Conventions

- Rust : `cargo fmt` et `clippy -D warnings` doivent passer. Pas de `unwrap()` / `expect()` hors tests et initialisation. Erreurs typées avec `thiserror` dans les modules, `anyhow` seulement aux frontières.
- Les commandes Tauri renvoient `Result<T, String>` ou une erreur sérialisable, jamais de panic.
- Les types partagés UI ↔ Rust sont définis côté Rust et dupliqués dans `src/lib/ipc.ts` (envisager `tauri-specta` si ça devient pénible).
- Tests unitaires obligatoires pour `player/queue.rs` (shuffle, repeat, file manuelle, précédent) : c'est de la logique pure, sans mpv ni réseau.
- Identifiants et code en anglais, commentaires et docs en français.
- Commits au format Conventional Commits (`feat(player): …`). Dépôt sur le Gitea perso (`origin`). Claude peut commiter et pousser seul, en le signalant dans son message.
- **Version** : `VERSION` à la racine est la source de vérité, affichée dans l'en-tête de l'app. `0.<phase>.<patch>` avant la 1.0 : patch à chaque lot de changements **qui modifie l'app livrée** (code, config, dépendances), mineure à la fin d'une phase. Pas de changement de version pour la seule documentation (`*.md`), sinon chaque correction de doc deviendrait une « mise à jour » une fois l'updater en place. `src-tauri/Cargo.toml` doit être mis à jour en même temps (`build.rs` fait échouer la compilation sinon) ; `tauri.conf.json` n'a pas de version et reprend celle de Cargo.
- Licence GPL-3.0 (`LICENSE`).

## Issues Gitea (pour Claude)

- Dépôt : `bebanana/Seaglass` sur `https://git.bebanana.ch` (Gitea 1.27). L'API exige une authentification.
- Accès via le serveur MCP officiel `gitea-mcp` (v1.11.0, installé dans `%LOCALAPPDATA%\Programs\gitea-mcp\`), déclaré dans `.mcp.json`. Le jeton vient de la variable d'environnement utilisateur `GITEA_ACCESS_TOKEN` : ne jamais l'écrire dans un fichier ni l'afficher. Outils utiles : `list_issues`, `issue_read` (`method: get` / commentaires), `issue_write` (commenter, labels), `pull_request_write`.
- Pour traiter une issue : la lire avec ses commentaires, poster un commentaire avec le plan avant de coder, travailler sur une branche `issue-<n>-<sujet>`, référencer l'issue dans le commit (`fix(ui): … (#8)`) et ouvrir une PR vers `main` avec `Closes #<n>`. Répondre dans l'issue si une question s'y pose plutôt que de deviner.

## Façon de travailler (pour Claude)

- **Ne pas accepter une proposition de l'utilisateur telle quelle** : à chaque proposition (fonctionnalité, outil, workflow, architecture), dire d'abord s'il existe une meilleure façon de faire ou une pratique plus standard, avec les raisons et une recommandation, puis avancer avec ce qu'il confirme.

- Fournir les **fichiers complets** modifiés, pas des patchs partiels.
- Commandes shell en PowerShell.
- Avant d'implémenter quoi que ce soit qui dépend d'un endpoint TIDAL, vérifier son comportement réel (spike ou requête de test) plutôt que de le supposer.
- Garder les modules petits : si un fichier dépasse environ 400 lignes, proposer un découpage.
- Mettre à jour la section « Statut » et la feuille de route de ce fichier à la fin de chaque phase.
