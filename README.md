# Seaglass

Client de streaming musical pour Windows, Android et Linux, avec une interface entièrement personnalisée et une lecture lossless / bit-perfect confiée à [mpv](https://mpv.io) (sur le bureau) ou à Media3 (sur Android).

> **Client non officiel, non affilié à TIDAL.** Un abonnement valide est nécessaire. Streaming uniquement : aucun flux audio n'est téléchargé ni enregistré sur disque, et les flux protégés par DRM ne sont pas lus.

Version actuelle : voir [`VERSION`](VERSION) ; nouveautés dans [`CHANGELOG.md`](CHANGELOG.md), aussi affiché dans l'app (Réglages › Application).

## Fonctionnalités

### Écoute

- Lecture FLAC jusqu'en 24 bits, avec l'édition hi-res d'un album quand elle existe ; sur Android, qualité « Haute » (AAC 320 kbit/s) au choix pour économiser les données.
- Enchaînement sans blanc (gapless) : le titre suivant est préchargé 30 s avant la fin.
- **Mode exclusif WASAPI** sous Windows : la sortie audio est réservée à Seaglass, sans mixage ni rééchantillonnage.
- Chaîne audio affichée en direct : format reçu, décodage, format envoyé à la carte son.
- Choix de la sortie audio, normalisation du volume, minuteur de mise en veille, pause quand le casque est débranché.
- File d'attente : contexte d'origine, « Lire ensuite », « Ajouter à la file », lecture aléatoire, répétition (tout / un titre), précédent, enregistrement de la file en playlist, bouton « Annuler » après un retrait.
- File, position et volume restaurés au démarrage, reprise de la lecture au lancement en option.
- Radio d'un titre, mixes et radio automatique en fin de file.

### Catalogue et bibliothèque

- Accueil personnalisé et réordonnable, recherche pendant la frappe (titres, albums, artistes, playlists) avec historique, pages album, artiste, playlist et mix.
- Bibliothèque : playlists, albums, artistes, titres likés, mixes, « Plus tard » et « Écoutés dans Seaglass » (historique d'écoute local).
- Gestion des playlists : créer, renommer, réordonner, supprimer.
- Paroles, crédits d'un titre ou d'un album, vue « En lecture » plein écran.
- Liens `tidal.com` collés ou touchés ouverts directement dans l'app.
- Épingles dans la barre latérale.

### Jam

Des invités ajoutent des titres à la file depuis leur téléphone, sans app ni compte : ils scannent un QR code et cherchent dans une page web. Fonctionne sur le réseau local ou en ligne par un relais (sans Wi-Fi commun), accepte les liens Spotify, Deezer, Apple Music et YouTube Music, et l'hôte garde la main (limites par invité, albums permis ou non, invités exclus).

### Interface

- Quinze thèmes : **Verre de mer** (verre dépoli sur la pochette, couleurs accordées à l'album), **Studio** (console sombre et dense), **Atelier** (clair, éditorial), **Acidwave** (néons acides et chrome), **Surréalisme** (paysage onirique teinté par l'album), **Solarpunk** (futur végétal et lumineux), **Cyberpunk** (interface de visée dans une ville néon), **Neumorphism** clair et sombre (reliefs doux ton sur ton), **Terminal** (écran cathodique monochrome), **Risographie** (fanzine deux encres), **Bauhaus** (affiche géométrique), **Aéro** (livrée d'avion) et **Alliage** clair et sombre (concept-car néo-rétro).
- Couleur d'accent réglable pour chaque thème, taille de l'interface (80 à 150 %), animations réduites ou complètes.
- Français et anglais, selon la langue du système ou au choix.
- Raccourcis : Espace, Ctrl+K, Ctrl+,, Échap, Alt+←, F11, Ctrl+« + » / « - » / 0.

### Intégration au système

- **Windows** : touches média et panneau multimédia (SMTC), icône de la zone de notification, lancement à l'ouverture de session, présence Discord.
- **Linux** : contrôles média par MPRIS (touches média, applet du bureau, `playerctl`, KDE Connect), présence Discord.
- **Android** : notification et session média (écouteurs, autoradio, montre), widget d'écran d'accueil, épingles en raccourcis de l'icône, mode conduite plein écran à grosses tuiles.
- Mises à jour automatiques signées sur les trois plateformes.
- Journaux et rapports de plantage gardés en local, copiables depuis Réglages › Application.

## Installation

Tout se télécharge sur **https://updates.bebanana.ch/seaglass/**. Ensuite, l'app se met à jour toute seule : quand une nouvelle version est prête, elle propose « Installer et redémarrer » ou « Plus tard ».

- **Windows 10 / 11** : lancer l'installeur. L'installation se fait pour l'utilisateur courant, sans droits administrateur, et mpv est inclus. L'installeur n'a pas de certificat de signature Windows : SmartScreen demande « Informations complémentaires » puis « Exécuter quand même ». Processeur compatible AVX2 requis par le build de mpv livré.
- **Android** (arm64) : installer l'APK en autorisant les sources inconnues.
- **Linux** (x86_64) : rendre l'AppImage exécutable et la lancer. mpv vient du système (`sudo apt install mpv`) ; les jetons sont rangés dans le trousseau (Secret Service). La connexion passe par le navigateur du système : coller dans l'app l'adresse de retour affichée.

## Prérequis (développement)

- Windows 10 ou 11 (WebView2 installé)
- [mpv](https://mpv.io/installation/) dans le `PATH` (utilisé en debug), ou son chemin dans la variable `SEAGLASS_MPV_PATH`
- [Rust](https://rustup.rs) stable et les Visual Studio Build Tools (C++)
- [Node.js](https://nodejs.org) LTS et [pnpm](https://pnpm.io)

Pour Android (SDK, NDK, test sur un appareil) et Linux (build dans WSL), voir [`docs/android.md`](docs/android.md) et [`docs/linux.md`](docs/linux.md).

## Démarrage

1. Créer un fichier `.env` à la racine (il est ignoré par git, et lu à la compilation) :

   ```
   TIDAL_CLIENT_ID=...
   TIDAL_CLIENT_SECRET=...
   ```

2. Installer les dépendances et lancer l'app en développement :

   ```powershell
   pnpm install
   pnpm tauri dev
   ```

3. Dans l'app, cliquer sur **Se connecter** et se connecter dans la fenêtre qui s'ouvre.

## Développement

```powershell
cd src-tauri; cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test
pnpm check
pnpm test
```

Aperçu de l'interface sans Tauri, sur des données factices : `pnpm dev`, puis `http://127.0.0.1:1420/?theme=studio&route=album` (voir [`docs/interface.md`](docs/interface.md), « Prévisualisation sans Tauri »).

L'architecture, les conventions et la feuille de route sont décrites dans [`CLAUDE.md`](CLAUDE.md), les notes d'implémentation par domaine dans [`docs/`](docs/) et les versions dans [`CHANGELOG.md`](CHANGELOG.md). Les bugs et demandes sont suivis dans les issues du dépôt Gitea ; chaque correction passe par une branche `issue-<n>-…` et une PR qui ferme l'issue. Le dossier `relay/` contient le relais de la Jam (déploiement dans [`relay/README.md`](relay/README.md)), et `spike/` le script Python de la phase 0 qui a validé les formats de flux.

## Publier une version

1. Monter `VERSION` (et `src-tauri/Cargo.toml`), ajouter l'entrée de `CHANGELOG.md`, commiter.
2. `pnpm release:all` : construit l'installeur Windows, l'APK et l'AppImage (dans WSL), signés, avec les manifestes de mise à jour, sans rien publier. `pnpm release` ne construit que l'installeur Windows.
3. `pnpm release:all -Publish` : publie dans le dossier des mises à jour, pose le tag `v<version>` et crée la release Gitea.

Détails, variables d'environnement et clés dans [`docs/release.md`](docs/release.md). Les clés privées de signature (updater et APK) doivent être sauvegardées : sans elles, les versions installées refuseront toute mise à jour.

## Licence

[GPL-3.0](LICENSE). mpv est lancé comme programme séparé et piloté par IPC, il n'est pas lié au code de Seaglass ; sa notice de licence est livrée avec l'installeur (`src-tauri/licenses/mpv.txt`).
