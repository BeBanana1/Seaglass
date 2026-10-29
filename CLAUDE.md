# Seaglass

Client desktop TIDAL non officiel, avec une UI entièrement personnalisée et une lecture lossless / bit-perfect déléguée à mpv.
Projet personnel, Windows en priorité.

> Statut : **phase 1 validée le 2026-10-29**. Connexion PKCE, recherche et lecture FLAC de bout en bout dans l'app. Prochaine étape : phase 2, file d'attente, gapless et persistance.

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

Demander avant d'ajouter une dépendance qui n'est pas dans cette liste.

## Arborescence cible

```
seaglass/
├─ CLAUDE.md
├─ spike/                    # phase 0 : scripts Python jetables
├─ src/                      # frontend Svelte
│  ├─ lib/ipc.ts             # wrappers typés des commandes et événements Tauri
│  ├─ lib/stores/            # état miroir du backend (player, queue, library)
│  ├─ components/
│  └─ views/                 # Home, Search, Album, Artist, Playlist, Library
└─ src-tauri/
   ├─ mpv/seaglass.conf        # config mpv dédiée à l'app
   └─ src/
      ├─ lib.rs              # setup Tauri, lancement des tâches
      ├─ commands.rs         # commandes Tauri : valident l'entrée puis envoient un Cmd
      ├─ tidal/              # auth.rs, api.rs, models.rs, manifest.rs (décodage + détection DRM)
      ├─ player/             # actor.rs, queue.rs, state.rs
      ├─ mpv/                # process.rs, ipc.rs, events.rs, serve.rs (MPD servis en HTTP local)
      ├─ store/              # SQLite
      └─ integrations/       # smtc.rs, discord.rs
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
- **Vers l'UI** : un seul événement `player-state` émis seulement si l'état a changé, avec `time_pos` limité à environ 4 Hz. L'UI interpole la barre de progression avec `requestAnimationFrame`.
- **Persistance** : file et position sauvegardées dans SQLite (debounce de 2 s), restaurées en pause au démarrage.

## mpv

- Lancé par l'app avec `--no-config --include=<seaglass.conf>` pour **ne jamais** charger la config mpv vidéo personnelle de l'utilisateur.
- Options de base : `--idle=yes --no-video --input-ipc-server=\\.\pipe\seaglass-mpv --ao=wasapi --gapless-audio=weak --prefetch-playlist=yes`.
- Mode bit-perfect (réglage utilisateur) : `--audio-exclusive=yes`, volume mpv à 100, pas de `replaygain`, pas de `audio-samplerate` forcé, pas de filtres audio.
- Propriétés observées : `playlist-pos`, `time-pos`, `duration`, `pause`, `paused-for-cache`, `audio-params`, `current-ao`, `idle-active`. Événement : `end-file`.
- Un micro-blanc lors d'un changement de fréquence d'échantillonnage en mode exclusif est **normal** (réouverture du périphérique). Ce n'est pas un bug à corriger.
- En dev : mpv dans le `PATH` ou chemin donné par `SEAGLASS_MPV_PATH`. En release : sidecar Tauri (`externalBin`).

## Feuille de route

0. ~~**Spike**~~ ✅ : script Python `spike/play.py` avec `tidalapi` : connexion, récupération du manifeste d'un titre, lecture dans mpv. Flux FLAC sans DRM confirmés avec le client PKCE.
1. ~~**Squelette**~~ ✅ : Tauri + Svelte, module `mpv/` (process + IPC + événements), commandes `login` (PKCE), `search`, `play`, `toggle_pause`, `seek`. Acteur lecteur minimal (un titre à la fois).
2. **(actuel)** File d'attente (`queue.rs`) + gapless + persistance, dans l'acteur existant.
3. Vues : accueil, recherche, album, artiste, playlist, bibliothèque.
4. Intégrations : SMTC, Discord RPC, Last.fm.
5. Thème, finitions, sidecar mpv, installeur.

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
- Vérifié en lecture réelle : `audio-codec-name=flac`, `current-ao=wasapi`, sortie `s16` 44,1 kHz pour un titre 16/44,1.

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
cd src-tauri; cargo fmt; cargo clippy -- -D warnings; cargo test
```

## Conventions

- Rust : `cargo fmt` et `clippy -D warnings` doivent passer. Pas de `unwrap()` / `expect()` hors tests et initialisation. Erreurs typées avec `thiserror` dans les modules, `anyhow` seulement aux frontières.
- Les commandes Tauri renvoient `Result<T, String>` ou une erreur sérialisable, jamais de panic.
- Les types partagés UI ↔ Rust sont définis côté Rust et dupliqués dans `src/lib/ipc.ts` (envisager `tauri-specta` si ça devient pénible).
- Tests unitaires obligatoires pour `player/queue.rs` (shuffle, repeat, file manuelle, précédent) : c'est de la logique pure, sans mpv ni réseau.
- Identifiants et code en anglais, commentaires et docs en français.
- Commits au format Conventional Commits (`feat(player): …`). Dépôt sur le Gitea perso.

## Façon de travailler (pour Claude)

- Fournir les **fichiers complets** modifiés, pas des patchs partiels.
- Commandes shell en PowerShell.
- Avant d'implémenter quoi que ce soit qui dépend d'un endpoint TIDAL, vérifier son comportement réel (spike ou requête de test) plutôt que de le supposer.
- Garder les modules petits : si un fichier dépasse environ 400 lignes, proposer un découpage.
- Mettre à jour la section « Statut » et la feuille de route de ce fichier à la fin de chaque phase.
