# Seaglass

Client desktop TIDAL non officiel, avec une UI entièrement personnalisée et une lecture lossless / bit-perfect déléguée à mpv.
Projet personnel, Windows en priorité.

> Statut : version courante dans `VERSION`, journal dans [`CHANGELOG.md`](CHANGELOG.md) (une version publiée porte un tag `v<version>`). Phases 0 à 5 livrées (mode exclusif WASAPI en 0.5.0, #245). App Android en cours (#24, [`docs/android.md`](docs/android.md)). Version Linux en cours (#17, [`docs/linux.md`](docs/linux.md)). Last.fm reporté, pas avant que l'utilisateur le demande.

Ce fichier ne garde que ce qui sert à toute tâche. Les notes d'implémentation sont dans `docs/` : voir « Documentation » plus bas et lire le fichier du domaine avant d'y toucher.

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
| Frontend | Svelte 5 + TypeScript + Vite, `pnpm`, tests `vitest` |
| Audio | mpv piloté en JSON IPC (named pipe), sidecar Tauri à terme |
| Stockage | SQLite (`rusqlite`) pour la file, les réglages et le cache de métadonnées |
| Secrets | `keyring` (Credential Manager sous Windows) |
| Intégrations | SMTC Windows (`windows` crate), MPRIS sous Linux (`zbus` 5, déjà tiré par les plugins, #312), icône de la zone de notification (`tauri`, feature `tray-icon`), présence Discord (IPC écrit à la main, #215), Last.fm (plus tard) |
| Mises à jour | `tauri-plugin-updater` (installeur NSIS signé, manifeste `latest.json`) |
| Système | `tauri-plugin-autostart` (lancement au démarrage, piloté uniquement côté Rust), `tauri-plugin-single-instance` (une seule instance, #83), `tauri-plugin-opener` (liens dans le navigateur, depuis Rust seulement, #188), `qrcode` sans ses options par défaut (QR code de la Jam en SVG, #25), `tokio-tungstenite` (connexion au relais de la Jam, #25 ; avec `rustls`, `rustls-platform-verifier` et `futures-util` en direct, déjà compilés par `reqwest`) |

Demander avant d'ajouter une dépendance qui n'est pas dans cette liste.

## Arborescence cible

```
seaglass/
├─ CLAUDE.md
├─ CHANGELOG.md              # journal des versions
├─ docs/                     # notes d'implémentation par domaine (voir « Documentation »)
├─ .gitea/workflows/ci.yml   # CI (runner dédié, voir ci/)
├─ ci/                       # image des jobs CI et runner act_runner (Docker, serveur)
├─ spike/                    # phase 0 : scripts Python jetables
├─ relay/                    # relais de la Jam (#25) : crate séparée, Docker derrière Caddy
├─ src/                      # frontend Svelte
│  ├─ lib/ipc.ts             # wrappers typés des commandes et événements Tauri
│  ├─ lib/transport.ts       # invoke/listen réels, ou backend factice hors Tauri (dev)
│  ├─ lib/mock/              # backend factice + fixtures.json (gitignoré) pour prévisualiser l'UI
│  ├─ lib/stores/            # état miroir du backend (player, queue, library, ui, nav)
│  ├─ lib/i18n.svelte.ts     # langue courante et dictionnaire `t` ; textes par domaine dans lib/i18n/
│  ├─ components/            # dont backdrops/ (fonds par thème)
│  ├─ themes/                # acidwave.css, surreal.css (surcharges par classe de composant)
│  └─ views/                 # Home, Search, Album, Artist, Playlist, Library
└─ src-tauri/
   ├─ mpv/seaglass.conf        # config mpv dédiée à l'app
   └─ src/
      ├─ lib.rs              # setup Tauri, lancement des tâches
      ├─ engine.rs           # contrat du moteur de lecture (`Op`, `Event`, trait `Engine`)
      ├─ media3.rs           # moteur Media3 d'Android (plugin `plugins/seaglass-mobile`)
      ├─ apk_update.rs       # mises à jour de l'app Android (manifeste `android.json`, APK)
      ├─ jam/                # Jam (#25) : api.rs (règles communes), server.rs (réseau local), relay.rs (en ligne), page/
      ├─ i18n.rs             # langue effective, messages traduits (`tr`, `trf!`), locale TIDAL
      ├─ commands/           # auth.rs, player.rs, catalog.rs, settings.rs : valident l'entrée puis délèguent
      ├─ tidal/              # auth.rs, api.rs, catalog.rs, home.rs (accueil paginé), playlist.rs (pages gardées), mix.rs (mixes, radios), hires.rs + twin.rs (éditions hi-res), credits.rs, raw.rs (JSON bruts), paging.rs (listes paginées), models.rs, cache.rs, manifest.rs (DRM)
      ├─ player/             # actor.rs, queue.rs, state.rs
      ├─ mpv/                # engine.rs (moteur mpv), process.rs, ipc.rs, events.rs, serve.rs (MPD servis en HTTP local)
      ├─ store/              # SQLite (dont snapshots.rs : copies des listes)
      ├─ foreign/            # liens d'autres services collés dans la Jam (#229) : links.rs, meta.rs, matching.rs
      └─ integrations/       # mod.rs (NowPlaying, Observers), smtc.rs, mpris/ (Linux), tray.rs, discord/ (présence Discord)
```

## Architecture du lecteur

- **Acteur unique.** Une seule tâche tokio possède `Player` (file + état). Tout passe par un `mpsc::Sender<Cmd>` : commandes UI, événements du moteur, SMTC. Pas de `Mutex` sur l'état du lecteur.
- **Moteur de lecture** (`engine.rs`, 0.4.42, #24) : l'acteur n'envoie que des opérations sémantiques (`Op::Play`, `Preload`, `RemoveNext`, `DropPrevious`, `Seek`, `LoopCurrent`, `Volume`, `CurrentGain`…) et reçoit des `Event` déjà traduits (`Position`, `Advanced`, `Ended(End::Eof | OutputFailed | Error)`, `Stopped`…). `mpv/engine.rs` (`MpvEngine`) traduit dans les deux sens (`commands`, `translate`, logique pure testée ; `cargo test live_engine_window -- --ignored --nocapture` avec un vrai mpv). Aucune commande JSON de mpv hors de `mpv/`. Le moteur Media3 d'Android implémentera le même trait `Engine`. `AudioDevice` et `CacheSize` restent dans `mpv` (réglages du bureau).
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

## Feuille de route

0. ~~**Spike**~~ ✅ : script Python `spike/play.py` avec `tidalapi` : connexion, récupération du manifeste d'un titre, lecture dans mpv. Flux FLAC sans DRM confirmés avec le client PKCE.
1. ~~**Squelette**~~ ✅ : Tauri + Svelte, module `mpv/` (process + IPC + événements), commandes `login` (PKCE), `search`, `play`, `toggle_pause`, `seek`. Acteur lecteur minimal (un titre à la fois).
2. ~~**File d'attente**~~ ✅ : `queue.rs` (testé), gapless, retry sur erreur, repeat One via `loop-file`, persistance SQLite.
3. ~~**Vues**~~ ✅ : accueil, recherche multi-types, album, artiste, playlist, bibliothèque (playlists, albums, artistes, titres).
4. ~~**Intégrations + UI**~~ ✅ : SMTC ; thèmes Verre de mer / Studio / Atelier / Acidwave / Surréalisme, vue « En lecture », chaîne audio, raccourcis clavier. Présence Discord en 0.4.92 (#215) ; Last.fm reporté (pas avant que l'utilisateur le demande).
5. ~~**Finitions**~~ ✅ : installeur NSIS, mpv embarqué + Job Object, mises à jour signées, script de release ; hébergement des mises à jour (v0.4.4 publiée) ; CI ; mixes et radios (0.4.8) ; mode exclusif WASAPI (0.5.0, #245).

## Documentation

Notes d'implémentation, lues à la demande : endpoints vérifiés, mesures, pièges, classes surchargées par les thèmes, procédures.

| Fichier | Contenu | À lire avant de toucher à |
|---|---|---|
| [`docs/lecteur.md`](docs/lecteur.md) | mpv (options, cache, normalisation, périphérique de sortie, AVX2), manifestes et pipe, file et persistance, commandes de la file, volume, minuteur, radio automatique, historique d'écoute, édition hi-res | `player/`, `mpv/`, `engine.rs`, `store/` |
| [`docs/tidal.md`](docs/tidal.md) | résultats du spike, connexion et garde DRM, endpoints du catalogue, fil d'actualité, mixes, paroles, écritures dans la bibliothèque, « Plus tard », infos du titre et crédits, performances du backend | `tidal/`, `commands/catalog.rs`, `commands/collection.rs` |
| [`docs/interface.md`](docs/interface.md) | navigation, réglages, raccourcis, aperçu sans Tauri (paramètres d'URL), CSP, pages et listes, menu contextuel, liens collés, performances des listes, langue | `src/` hors thèmes, `commands/settings/`, `i18n.rs` |
| [`docs/themes.md`](docs/themes.md) | jetons, fonds, couleur d'accent, cascade, notes de chaque thème, classes surchargées | `src/themes/`, `app.css`, `components/backdrops/`, et avant de renommer une classe de composant |
| [`docs/systeme.md`](docs/systeme.md) | SMTC, MPRIS, présence Discord, zone de notification, lancement au démarrage | `integrations/` (bureau) |
| [`docs/linux.md`](docs/linux.md) | ce qui diffère sous Linux (sortie audio, arrêt de mpv, exclusif), construire et tester dans WSL | le code `cfg(not(windows))` du bureau, un paquet Linux |
| [`docs/android.md`](docs/android.md) | état de l'app Android, moteur Media3, session média, pont natif, secrets, mise en page tactile, distribution de l'APK, construire et tester sur le S21 | `media3.rs`, `android.rs`, `plugins/seaglass-mobile/`, `gen/android/`, les styles `data-platform="android"` |
| [`docs/jam.md`](docs/jam.md) | Jam locale et en ligne, API des invités, garde-fous, relais, liens d'autres services | `jam/`, `foreign/`, `relay/` |
| [`docs/release.md`](docs/release.md) | installeur NSIS, mpv embarqué, mises à jour, signature, scripts de release, release Gitea, CI et runner | `scripts/`, `updater.rs`, `tauri*.conf.json`, `.gitea/`, `ci/` |
| [`docs/audits.md`](docs/audits.md) | audits passés et ce qu'ils ont écarté | une question de sécurité ou de performances déjà tranchée |

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
pnpm test                                         # tests du frontend (vitest)
pnpm release                                      # installeur signé dans release/ (voir docs/release.md)
pnpm release:all -Publish                         # Windows + Android + Linux (WSL), tag, release Gitea
```

## Conventions

- Rust : `cargo fmt` et `clippy -D warnings` doivent passer. Pas de `unwrap()` / `expect()` hors tests et initialisation. Erreurs typées avec `thiserror` dans les modules, `anyhow` seulement aux frontières.
- Les commandes Tauri renvoient `Result<T, String>` ou une erreur sérialisable, jamais de panic.
- Les types partagés UI ↔ Rust sont définis côté Rust et dupliqués dans `src/lib/ipc.ts` (envisager `tauri-specta` si ça devient pénible).
- Tests unitaires obligatoires pour `player/queue.rs` (shuffle, repeat, file manuelle, précédent) : c'est de la logique pure, sans mpv ni réseau.
- Identifiants et code en anglais, commentaires et docs en français.
- Commits au format Conventional Commits (`feat(player): …`). Dépôt sur le Gitea perso (`origin`). Claude peut commiter et pousser seul, en le signalant dans son message.
- **Version** : `VERSION` à la racine est la source de vérité, affichée dans l'en-tête de l'app. `0.<phase>.<patch>` avant la 1.0 : patch à chaque lot de changements **qui modifie l'app livrée** (code, config, dépendances), mineure à la fin d'une phase. Pas de changement de version pour la seule documentation (`*.md`), sinon chaque correction de doc deviendrait une « mise à jour » une fois l'updater en place. La CI le vérifie (« Contrôle de version » dans `docs/release.md`). `src-tauri/Cargo.toml` doit être mis à jour en même temps (`build.rs` fait échouer la compilation sinon) ; `tauri.conf.json` n'a pas de version et reprend celle de Cargo.
- Licence GPL-3.0 (`LICENSE`).

## Issues Gitea (pour Claude)

- Dépôt : `bebanana/Seaglass` sur `https://git.bebanana.ch` (Gitea 1.27). L'API exige une authentification.
- Accès via le serveur MCP officiel `gitea-mcp` (v1.11.0, installé dans `%LOCALAPPDATA%\Programs\gitea-mcp\`), déclaré dans `.mcp.json`. Il agit sous le compte dédié `claude` (collaborateur en écriture du dépôt), pour que ses issues, commentaires et PR se distinguent de ceux de l'utilisateur et lui soient notifiés. Le jeton de ce compte vient de la variable d'environnement utilisateur `GITEA_CLAUDE_TOKEN` (droits `write:issue`, `write:repository`, `read:user`) : ne jamais l'écrire dans un fichier ni l'afficher. Le jeton personnel de l'utilisateur n'est pas utilisé par Claude : si les outils Gitea échouent (variable absente, jeton révoqué), le signaler à l'utilisateur plutôt que de revenir à `GITEA_ACCESS_TOKEN` ou de passer par l'API avec un autre jeton. Vérification : `get_me` doit renvoyer `claude`. Les commits restent à l'identité git de l'utilisateur, avec la ligne `Co-Authored-By` ; la fusion des PR revient à l'utilisateur. Outils utiles : `list_issues`, `issue_read` (`method: get` / commentaires), `issue_write` (commenter, labels), `pull_request_write`.
- **Labels** (2026-11-04, labels à portée de Gitea) : `platform/android` | `platform/windows` (exclusif ; aucun = les deux), `type/bug` | `feature` | `improvement` | `perf` | `security` | `chore` | `docs` (exclusif), `area/player` | `tidal` | `ui` | `theme` | `library` | `release` | `ci` (plusieurs possibles), `status/needs-info` | `blocked` | `wontdo` | `duplicate` (exclusif). Toute issue créée ou traitée reçoit au moins un `type/` et une `area/` ; `status/needs-info` quand elle attend une réponse de l'utilisateur, retiré quand il a répondu ; `status/wontdo` sur une proposition écartée.
- Pour traiter une issue : la lire avec ses commentaires, poster un commentaire avec le plan avant de coder, travailler sur une branche `issue-<n>-<sujet>`, référencer l'issue dans le commit (`fix(ui): … (#8)`) et ouvrir une PR vers `main` avec `Closes #<n>`. Répondre dans l'issue si une question s'y pose plutôt que de deviner.
- **Plusieurs issues à la suite** : ouvrir la PR de chaque issue dès que sa branche est vérifiée et poussée, avant de passer à la suivante, pour que les jobs de CI s'étalent au lieu de s'enchaîner à la fin. Chaque lot monte `VERSION` et `CHANGELOG.md` : partir de la branche précédente plutôt que de `main` (PR empilées, sinon elles entrent en conflit) et indiquer l'ordre de fusion dans chaque description.

## Façon de travailler (pour Claude)

- **Ne pas accepter une proposition de l'utilisateur telle quelle** : à chaque proposition (fonctionnalité, outil, workflow, architecture), dire d'abord s'il existe une meilleure façon de faire ou une pratique plus standard, avec les raisons et une recommandation, puis avancer avec ce qu'il confirme.

- Fournir les **fichiers complets** modifiés, pas des patchs partiels.
- Commandes shell en PowerShell.
- Avant d'implémenter quoi que ce soit qui dépend d'un endpoint TIDAL, vérifier son comportement réel (spike ou requête de test) plutôt que de le supposer.
- Garder les modules petits : si un fichier dépasse environ 400 lignes, proposer un découpage.
- **Documentation** : ce fichier reste court. Ce qu'on apprend en traitant une issue (endpoint vérifié, mesure, piège, décision et sa raison) va dans le fichier de `docs/` du domaine, dans la même PR. Corriger ou retirer ce qui est périmé plutôt que d'ajouter à la suite.
- **Journal** : chaque changement de `VERSION` ajoute son entrée en tête de `CHANGELOG.md` (une ligne, avec le numéro de l'issue). Après une release, le titre de la version taguée reçoit « (publiée le <date>) ».
- Mettre à jour la ligne « Statut » et la feuille de route de ce fichier à la fin de chaque phase.
