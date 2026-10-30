# Seaglass

Client desktop TIDAL non officiel, avec une UI entièrement personnalisée et une lecture lossless / bit-perfect déléguée à mpv.
Projet personnel, Windows en priorité.

> Statut : **v0.4.6 publiée** (2026-10-30) sur `https://updates.bebanana.ch/seaglass/` : installeur NSIS signé, mpv embarqué, mises à jour automatiques ; page Réglages, volume, cache mpv, lancement au démarrage, accueil personnalisable, filtre et tri des listes, ajout d'albums à la file, file dans « En lecture ». Phase 4 livrée (SMTC, cinq thèmes) ; Discord RPC et Last.fm reportés à la demande de l'utilisateur. CI en service (runner dédié). Reste en phase 5 : mode exclusif WASAPI. **0.4.6 publiée** : playlists épinglées (#10), couleur d'accent (#23), vider la file (#41), bouton « Vérifier les mises à jour » (#43). **0.4.7 publiée** : installeur en français, page de téléchargement à `https://updates.bebanana.ch/seaglass/`. **0.4.8** (non publiée) : mixes et radios (#11), fil d'actualité (#14), paroles synchronisées (#5).

## Principes non négociables

1. **Streaming uniquement.** Aucun téléchargement, export ou mise en cache des flux audio sur disque. Aucun contournement de DRM : si un flux est protégé par Widevine, on ne le lit pas, on remonte une erreur claire.
2. **Rust est la source de vérité.** L'UI n'a aucune logique métier : elle affiche l'état reçu du backend et envoie des commandes. Elle ne parle jamais directement à TIDAL ni à mpv.
3. **Tout ce qui touche TIDAL vit dans `src-tauri/src/tidal/`.** C'est une API privée non documentée qui peut changer à tout moment : le reste du code ne dépend que des types de `tidal::models`, jamais des formats JSON bruts.
4. **Aucun secret dans le dépôt.** Client ID / secret dans `.env` (gitignoré) ou variables d'environnement. Tokens utilisateur dans le Gestionnaire d'identifiants Windows via la crate `keyring`. Ne jamais logger un token, même tronqué.
5. **Pas de nom ni de logo TIDAL** dans le nom de l'app, l'icône ou le titre de fenêtre. Mention « Client non officiel, non affilié à TIDAL » dans Réglages › Application et le README.

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
| Système | `tauri-plugin-autostart` (lancement au démarrage, piloté uniquement côté Rust) |

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
      ├─ tidal/              # auth.rs, api.rs, catalog.rs, mix.rs (mixes, radios), raw.rs (JSON bruts), models.rs, cache.rs, manifest.rs (DRM)
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
- Propriétés observées : `playlist-pos`, `time-pos`, `duration`, `pause`, `paused-for-cache`, `audio-params`, `current-ao`, `idle-active`. Événement : `end-file`. Propriétés écrites sans être observées (l'état de l'acteur fait foi) : `volume`, `mute`, `demuxer-max-bytes`, `demuxer-max-back-bytes`.
- **Cache** : `cache=yes` et `cache-on-disk=no` dans `seaglass.conf` (mémoire uniquement). La taille vient du réglage `playback.settings` (`mpv/cache.rs`, `player/settings.rs` : 32 / 64 / 128 / 256 Mio, défaut 64, retour = un quart), passée en ligne de commande au lancement puis appliquée à chaud par `set_property`. Effectif dès que la lecture consomme des paquets (en pause, le démuxeur plein reste endormi) ; vérifié par `cargo test live_cache_resize -- --ignored --nocapture`. Avec la suivante préchargée, la mémoire peut atteindre deux fois la taille choisie.
- Un micro-blanc lors d'un changement de fréquence d'échantillonnage en mode exclusif est **normal** (réouverture du périphérique). Ce n'est pas un bug à corriger.
- Résolution de mpv (`mpv::process::executable`) : `SEAGLASS_MPV_PATH`, sinon `mpv.exe` à côté de `seaglass.exe` (installé par l'installeur), sinon celui du `PATH` (dev).
- mpv est rattaché à un Job Object Windows `KILL_ON_JOB_CLOSE` (`mpv/job.rs`) : il meurt avec l'app, même sur un plantage.

## Feuille de route

0. ~~**Spike**~~ ✅ : script Python `spike/play.py` avec `tidalapi` : connexion, récupération du manifeste d'un titre, lecture dans mpv. Flux FLAC sans DRM confirmés avec le client PKCE.
1. ~~**Squelette**~~ ✅ : Tauri + Svelte, module `mpv/` (process + IPC + événements), commandes `login` (PKCE), `search`, `play`, `toggle_pause`, `seek`. Acteur lecteur minimal (un titre à la fois).
2. ~~**File d'attente**~~ ✅ : `queue.rs` (testé), gapless, retry sur erreur, repeat One via `loop-file`, persistance SQLite.
3. ~~**Vues**~~ ✅ : accueil, recherche multi-types, album, artiste, playlist, bibliothèque (playlists, albums, artistes, titres).
4. ~~**Intégrations + UI**~~ ✅ : SMTC ; thèmes Verre de mer / Studio / Atelier / Acidwave / Surréalisme, vue « En lecture », chaîne audio, raccourcis clavier. Discord RPC et Last.fm reportés (pas avant que l'utilisateur le demande).
5. **(actuel)** Finitions : ~~installeur NSIS, mpv embarqué + Job Object, mises à jour signées, script de release~~ ✅ ; ~~hébergement des mises à jour~~ ✅ (v0.4.4 publiée) ; ~~CI~~ ✅ ; ~~mixes et radios~~ ✅ (0.4.8) ; reste le mode exclusif WASAPI (réglage).

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
- **Accueil v2** : lignes `{type, title, items: [{type, data}]}`. Seuls `ALBUM`, `ARTIST`, `PLAYLIST` et `MIX` (depuis 0.4.8) sont gardés ; `TRACK` (sans album) et `DEEP_LINK` sont ignorés. Avec `locale=fr_FR`, les titres de lignes sont en français. Chaque ligne a un `moduleId` stable, indépendant de la langue (`BECAUSE_YOU_LISTENED_TO_ALBUM`, `YOUR_FAVORITE_ARTISTS`…) → `HomeRow::key` ; « Parce que vous avez écouté » porte son contexte dans un `header` séparé. La réponse a un `page.cursor` (pagination) que l'app ne suit pas encore.
- **Date d'ajout** : chaque titre de `playlists/{uuid}/tracks` porte `dateAdded` (`2024-04-12T22:51:56.872+0000`), les favoris `created` (date du like). Les deux deviennent `TrackEntry::added_at` (ms UTC, `tidal/date.rs`). `TrackEntry` est sérialisé à plat (`#[serde(flatten)]`) : côté UI, un `Track` avec `addedAt`. La date ne va ni dans le cache ni dans la file.
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
- **Raccourcis** : Espace (lecture/pause), Ctrl+K ou `/` (recherche), Échap (fermer), Alt+← (retour), Ctrl+, (réglages), Ctrl+↑ / Ctrl+↓ (volume ±5).
- **Prévisualisation sans Tauri** : `pnpm dev` puis `http://127.0.0.1:1420/?theme=studio&route=album&nowplaying=1` (paramètres : `theme`, `route` dont `settings`, `mix` et `radio`, `tab` (onglet de bibliothèque ou section des réglages `appearance|playback|app`), `nowplaying`, `queue`, `paused`, `loggedout`, `update`, `autostart`, `volume=60|muted`, `accent=ff5c8a`, `lyrics=1|plain|none`). Plusieurs serveurs en parallèle : `pnpm vite --port <n> --strictPort`. Svelte 5 délègue `change` et `input` : un événement créé par script doit avoir `bubbles: true`. Données de `src/lib/mock/fixtures.json`, généré par `cargo test dump_ui_fixtures -- --ignored` (gitignoré : contient la bibliothèque du compte). La branche factice est éliminée du build de production (`import.meta.env.DEV`).
- **Piège Vite** : à deux reprises, après des éditions de fichiers par script, le CSS d'un composant est resté périmé alors que son JS était à jour (règles absentes du `?svelte&type=style`). Redémarrer `pnpm tauri dev` et vider `node_modules/.vite` règle le problème. Le panneau de navigateur intégré, quand il est masqué, gèle aussi les animations et le chargement différé des images : pour une capture, injecter `*{animation:none!important}`.

## Implémentation (phase 5 : installeur et mises à jour)

- **Installeur** : NSIS uniquement (`bundle.targets`), installation par utilisateur (`installMode: currentUser`, pas d'admin), dans `%LOCALAPPDATA%\Seaglass`, en français (`nsis.languages`, depuis 0.4.7). Le modèle NSIS de Tauri crée toujours le raccourci du menu Démarrer ; sa page de fin propose « Lancer » et le raccourci sur le bureau. Pas de signature Authenticode : SmartScreen avertit au premier lancement (seul remède : un certificat payant, par exemple Azure Trusted Signing). Icône : `src-tauri/icons/app-icon.svg`, déclinée avec `pnpm tauri icon src-tauri/icons/app-icon.svg` (supprimer ensuite `icons/android` et `icons/ios`). Licences livrées dans `licenses/` (GPL-3.0 de Seaglass, notice mpv).
- **mpv embarqué** : seulement au build de release, via `src-tauri/tauri.release.conf.json` (`externalBin: binaries/mpv`). Le script copie le mpv du `PATH` en `src-tauri/binaries/mpv-x86_64-pc-windows-msvc.exe` (gitignoré, 119 Mo). Dans la config de base, `externalBin` ferait échouer tout `cargo build` sans ce fichier. Le build actuel de mpv (zhongfly, x86-64-v3) exige un processeur AVX2.
- **Mises à jour** (`src-tauri/src/updater.rs`) : vérification 20 s après le démarrage puis toutes les 6 h ; téléchargement en arrière-plan, signature vérifiée par le plugin ; événement `update-ready` → `UpdatePrompt.svelte` (« Installer et redémarrer » = `install_update`, « Plus tard » = installation sur `RunEvent::Exit`, sans relance). `on_before_exit` arrête mpv avant que l'installeur ferme l'app. Rien n'est vérifié automatiquement en debug sauf si `SEAGLASS_UPDATE_URL` est défini (aussi utile pour tester un autre manifeste). Bouton « Vérifier » dans Réglages › Application (0.4.6, #43) : `check_for_updates` lance la même vérification, sérialisée avec la tâche de fond (`UpdateState::checking`), et réémet `update-ready` si une version est prête, même déjà reportée. Il fonctionne aussi en debug (manifeste de production par défaut).
- **Signature** : clé minisign dans `%USERPROFILE%\.tauri\seaglass.key` (sans mot de passe, hors du dépôt), clé publique dans `tauri.conf.json` (`plugins.updater.pubkey`). **Perdre la clé privée** = plus aucune mise à jour acceptée par les versions installées (réinstallation manuelle) : elle doit être sauvegardée.
- **Hébergement** (en service) : le Gitea exige une connexion partout, donc l'updater ne peut pas y lire de releases. Les mises à jour sont servies par le Caddy du serveur (Docker) : `updates.bebanana.ch` (CNAME vers `bebanana.internet-box.ch`) sert `/srv/updates` en lecture seule, `latest.json` en `Cache-Control: no-cache`. Le dossier est public, les installeurs étant signés.
- **Publication** : utilisateur `seaglass-deploy` (sans mot de passe ni sudo, clé SSH avec `restrict`), alias `seaglass-updates` dans `~/.ssh/config` du PC (IP locale du serveur), `SEAGLASS_RELEASE_DEST=seaglass-updates:/srv/updates/seaglass` (variable utilisateur Windows). Après `scp`, le script remet les droits à 755 / 644 : sinon le dossier de version arrive en écriture pour tous.
- **Release** : `pnpm release` (`scripts/release.ps1`) vérifie l'arbre git, `.env`, la clé et `VERSION`, embarque mpv, lance `pnpm tauri build --config src-tauri/tauri.release.conf.json`, puis écrit `release/latest.json`, `release/<version>/Seaglass_<version>_x64-setup.exe` et la page de téléchargement `release/index.html` (modèle `scripts/download-page.html`, champs `{{…}}` échappés en HTML, nouveautés tirées des notes). Avec `-Publish`, il copie vers `SEAGLASS_RELEASE_DEST` (dossier ou cible scp) l'installeur, puis la page, le manifeste en dernier, puis pose et pousse le tag `v<version>`. Une release = un tag : pas de mise à jour à chaque push.

## Implémentation (phase 5 : CI)

- **Workflow** : `.gitea/workflows/ci.yml`, sur push vers `main` et sur chaque PR, sauf si seuls des `*.md` / `LICENSE` changent : contrôle de version, `pnpm check`, `pnpm build`, `cargo fmt --check`, `clippy -D warnings`, `cargo test`. `pnpm build` passe avant Rust car `tauri::generate_context!` exige `dist/`. Ni `.env` ni `fixtures.json` ne sont nécessaires.
- **Contrôle de version** (`scripts/check-version.sh`, checkout avec `fetch-depth: 0`) : échoue si `VERSION` est inférieure au dernier tag `v*`, ou si elle lui est égale alors que des fichiers de l'app ont changé depuis ce tag (hors `*.md`, `LICENSE`, `.gitea/`, `ci/`, `scripts/`, `spike/`, `.claude/`, `.mcp.json`, `.gitignore`, `.gitattributes`, `src/lib/mock/`). Un changement de tests Rust seuls demande donc aussi une nouvelle version. Lançable en local : `bash scripts/check-version.sh`. Pas de publication automatique : la clé de signature reste sur le PC et l'installeur se construit sous Windows.
- **Runner dédié** (`ci/`, installation dans `ci/README.md`) : `act_runner` 0.6.1 en Docker sur le serveur, enregistré **au niveau du dépôt**, label `seaglass`. Il monte le socket Docker, donc il équivaut à root sur le serveur : ne jamais l'ouvrir à d'autres dépôts. Le runner partagé (firmware, KiCad) n'est plus utilisé.
- **Image des jobs** `seaglass-ci:<tag daté>` (`ci/Dockerfile`, base `node:24-bookworm`) : bibliothèques de Tauri, Rust stable + rustfmt + clippy, pnpm. Construite sur le serveur (`docker compose --profile build build`), sans registre (`force_pull: false`). Changer de tag = `.env` + label de `config.yaml`.
- **Cache** : les jobs rejoignent le réseau du runner (`container.network: seaglass-ci`) et le cache écoute sur un port fixe (8088). Sans ça, chaque action de cache attend 5 min un `ETIMEDOUT` (vu sur le runner partagé).
- **Limite** : sous Linux, le code `#[cfg(windows)]` (SMTC, Job Object, `CREATE_NO_WINDOW`) n'est ni compilé ni testé ; `pnpm release` le vérifie sous Windows. Du code partagé utilisé seulement par une intégration Windows doit tolérer le code mort hors Windows (`#[cfg_attr(not(windows), allow(dead_code))]`, voir `integrations/mod.rs` et `Cmd::SetPaused`), sinon clippy échoue en CI. L'IPC mpv passe par un socket Unix hors Windows (`mpv::ipc::open`).
- **Durée** : environ 7 min sur le runner partagé, sans cache (préparation ~4 min, clippy ~1 min, tests ~1 min 30). Runner dédié (2026-10-30) : 5 min sans cache (Rust ~2 min 40, sauvegarde du cache ~1 min 40), 4 min 30 au premier run de `main` (cache restauré en 35 s mais clé partielle, Rust ~1 min 15, nouvelle sauvegarde ~2 min 20). Un run dont la clé correspond exactement ne sauvegarde pas : durée en régime établi à confirmer.
- **Mise en service** : si un job démarre sur `runner-images:ubuntu-latest` avec un réseau `…-check-network`, le runner ne lit pas `config.yaml` (conteneur lancé depuis d'anciens fichiers). Le `data/.runner` appartient à root : `sudo rm` pour réenregistrer.
- **Pas de release en CI** : la clé minisign reste sur le PC, `pnpm release` reste manuel.

## Implémentation (phase 5 : lot d'issues, 0.4.5)

- **Page Réglages** (#9, `views/Settings.svelte`) : route `settings`, sections Apparence / Lecture / Application. Entrées : barre latérale, bouton de l'en-tête, `Ctrl+,`. Accessible déconnecté (Retour ramène à la connexion). Plus de boîtes Apparence / À propos : `ThemePicker.svelte` et la section Application les remplacent.
- **Lancement au démarrage** (#13) : `tauri-plugin-autostart` piloté côté Rust (`autostart_enabled` / `set_autostart`), sans paquet JS ni capability. L'état est lu dans le registre (`HKCU\…\Run`, valeur `Seaglass`, et `StartupApproved\Run`), jamais copié en SQLite. Chemin = `current_exe()` : à tester sur un build installé. Limites connues : `auto-launch` tente `HKLM` en premier (app lancée en admin), valeur non entre guillemets, entrée non retirée à la désinstallation, fenêtre ouverte normalement.
- **Volume** (#29, `player/volume.rs`) : `set_volume` (0–100) / `set_muted` → `volume` / `mute` de mpv, appliqués aussi au démarrage. mpv applique un gain cubique : pas de courbe côté app, 100 = gain unitaire. Persisté dans `kv` (`player.volume`) avec la cadence de 2 s, relu même sans file. Bouger le curseur rétablit le son ; rétablir à 0 repart de 50. Volume ≠ 100 ou coupé → verdict « Volume logiciel » / « Son coupé » dans la chaîne audio, jamais bit-perfect.
- **Ajout en lot** (#15) : `enqueue_tracks(trackIds, next)` ajoute à la file manuelle dans l'ordre donné (« Lire ensuite » = `push_front` en partant du dernier, sinon l'album serait inversé). Boutons dans l'en-tête Album et Playlist (`PageHeader` `queueable`). Tests de la file dans `player/queue/tests.rs`.
- **File dans « En lecture »** (#20) : `components/QueueList.svelte`, partagé avec la vue File. Panneau replié par défaut, état de session `ui.nowPlayingQueue`. Toute navigation ferme la vue « En lecture ».
- **Accueil personnalisable** (#21, #22) : `commands/settings/home_layout.rs` (logique pure, testée), réglage `UiSettings::home { order, hidden }`, commandes `set_home_layout` (sections dans l'ordre de l'écran ; les absentes du flux gardent leur place) et `reset_home_layout`. `home` renvoie des `HomeSection` (avec `hidden`) : l'UI masque hors du mode « Personnaliser ». Masquer porte sur le type de section.
- **Filtre et tri des listes** (#1, #2, `lib/trackOrder.ts`, `components/TrackTools.svelte`) : présentation sur une liste déjà chargée, dans Playlist et Bibliothèque › Titres. Lire depuis une liste filtrée ou triée met en contexte la liste affichée, dans l'ordre affiché ; Lecture / Aléatoire de l'en-tête suivent le tri mais pas le filtre. `TrackList` accepte `positions` pour garder les numéros d'origine. Ni tri ni filtre mémorisés.
- **Épingles** (0.4.6, #10, `commands/settings/pins.rs`, logique pure testée) : `UiSettings::pins`, liste ordonnée de `Pin` (`kind: playlist` avec uuid + titre, ou `likedTracks`). `set_pins` valide (uuid `8-4-4-4-12`, doublons retirés, 50 maximum) et renvoie la liste enregistrée. Bouton `PinButton` dans l'en-tête Playlist et dans Bibliothèque › Titres ; section « Épinglés » sous Bibliothèque dans `Sidebar`. Le titre gardé est remis à jour à l'ouverture de la playlist. Backend factice : les arguments sont des proxys Svelte, un handler qui les renvoie doit les copier en JSON (`structuredClone` les refuse).
- **Vider la file** (0.4.6, #41) : `clear_queue` → `Queue::clear_upcoming` vide la file manuelle et le contexte ; le titre courant continue, détaché du contexte (rien ne le suit, même en repeat All), l'historique reste. Bouton « Vider la file » en tête de `QueueList`.
- **Couleur d'accent** (0.4.6, #23) : `UiSettings::accent` (`#rrggbb` en minuscules ou `null`), validée à la désérialisation. L'UI pose `--accent`, `--accent-strong` (mêlée à `--text`) et `--accent-contrast` (noir ou blanc selon la luminance) en style en ligne sur `<html>`, ce qui l'emporte sur les jetons du thème ; `null` les retire. Pour qu'un thème suive l'accent choisi, ses couleurs dérivées doivent passer par `var(--accent)` (`color-mix(in srgb, var(--accent) N%, transparent)`, pas de `rgb()` codé en dur) et le texte posé sur l'accent par `var(--accent-contrast)`. Les miniatures de `ThemePicker` gardent leurs couleurs d'origine. Aperçu : `?accent=ff5c8a`.
- **Fil d'actualité** (0.4.8, #14, `tidal/feed.rs`, vue `views/Feed.svelte`, route `feed`) : v2 `feed/activities?userId=…&deviceType=BROWSER&platform=WEB&locale=fr_FR` → `{ activities: [{ followableActivity: { activityType, occurredAt, album | historyMix }, seen }], stats }`. Types vus : `NEW_ALBUM_RELEASE` (album complet) et `NEW_HISTORY_MIX` (ignoré tant que les mixes ne sont pas lisibles). **Pas de pagination** : `offset` ignoré, `limit` tronque, fenêtre d'environ 50 activités (vérifié le 2026-10-30). `seen` est lu (pastille « Nouveau »), jamais écrit. L'UI groupe par Aujourd'hui / Cette semaine / mois et réutilise `Card`. Aperçu : `?route=feed`.
- **Mixes et radios** (0.4.8, #11, `tidal/mix.rs`) : `MediaItem::Mix(MixSummary)` (id alphanumérique, titre, sous-titre, image 320, `mixType`). Un même mix arrive sous trois formes, toutes lues par `RawMix` (tests) : accueil v2 (`titleTextInfo.text`, `subtitleTextInfo.text`, `mixImages: [{size, url}]`, `type`), v2 `favorites/mixes` et `pages/mix` (`title`, `subTitle`, `images: {SMALL|MEDIUM|LARGE: {url}}`, `mixType`). URLs d'image signées par l'API (`images.tidal.com/…?token=`), gardées telles quelles. Commandes `mix(id)` (en-tête de `pages/mix?mixId=` + titres de `mixes/{id}/items`, éléments `{item, type}` dont seuls les `track` sont gardés), `radio(seed: track|artist, id)` (`tracks/{id}/radio` : 96 titres vus, `artists/{id}/radio` : 100) et `library_mixes` (v2 `favorites/mixes`, curseur ; 14 mixes sur le compte de test). Les mixes ne vont pas dans le cache (contenu changeant), leurs titres oui. UI : vue `Mix.svelte` partagée par les routes `mix` et `radio` ; bouton Radio sur chaque ligne de titre (`TrackRow`) et dans l'en-tête d'un artiste (`PageHeader` `onRadio`) ; onglet « Mixes » de la bibliothèque. Pas encore : enregistrer un mix, radio qui se prolonge en fin de liste.
- **Paroles** (0.4.8, #5, `tidal/lyrics.rs`) : `GET v1/tracks/{id}/lyrics` (vérifié, test `cargo test live_lyrics -- --ignored --nocapture`) renvoie `{lyricsProvider, isRightToLeft, lyrics, subtitles}`, `subtitles` en LRC ; **404 = pas de paroles** (instrumental, classique), traduit en `None`. `Lyrics::text` = `Synced` (lignes en ms, triées, ligne vide = passage instrumental) ou `Plain` (repli si `subtitles` est vide ou illisible). Parsing LRC testé : horodatages multiples sur une ligne, `mm:ss[.x|.xx|.xxx]`, en-têtes `[ar:…]` ignorés. Commande `lyrics(trackId)`. UI : `components/Lyrics.svelte` dans un panneau de « En lecture », exclusif avec la file (`ui.nowPlayingPanel: "queue" | "lyrics" | null`). La ligne courante suit `estimatedPosition` (comme `Progress`) et reste centrée par défilement doux, suivi suspendu 4 s après un défilement manuel ; clic sur une ligne = seek. Marges de défilement en `cqh` (`container-type: size`) : des `vh` ou `%` débordaient du panneau. Crédit du fournisseur (Musixmatch) sous les paroles.
- **Thèmes** : nouvelles classes surchargées par Acidwave / Surréalisme : `.theme-card`, `.cache-sizes`, `.cache-size`, `.setting-row`, `.setting-switch`, `.volume`, `.mute`, `.home-customize`, `.home-editor-*`, `.queue-toggle`, `.track-tools`, `.pins`, `.pin`, `.pin-open`, `.pins-title`, `.pin-button`, `.accent-theme`, `.accent-swatch`, `.accent-custom`, `.update-check`, `.queue-clear`, `.feed-new`, `.lyrics-line`, `.lyrics-toggle`, `.lyrics-credit`.

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
- **Version** : `VERSION` à la racine est la source de vérité, affichée dans l'en-tête de l'app. `0.<phase>.<patch>` avant la 1.0 : patch à chaque lot de changements **qui modifie l'app livrée** (code, config, dépendances), mineure à la fin d'une phase. Pas de changement de version pour la seule documentation (`*.md`), sinon chaque correction de doc deviendrait une « mise à jour » une fois l'updater en place. La CI le vérifie (voir « Contrôle de version »). `src-tauri/Cargo.toml` doit être mis à jour en même temps (`build.rs` fait échouer la compilation sinon) ; `tauri.conf.json` n'a pas de version et reprend celle de Cargo.
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
