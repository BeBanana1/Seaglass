# Seaglass

Client de streaming musical pour Windows, Android et Linux, avec une interface entièrement personnalisée et une lecture lossless / bit-perfect confiée à [mpv](https://mpv.io) (sur le bureau) ou à Media3 (sur Android).

> **Client non officiel, non affilié à TIDAL.** Un abonnement valide est nécessaire. Streaming uniquement : aucun flux audio n'est téléchargé ni enregistré sur disque, et les flux protégés par DRM ne sont pas lus.

Version actuelle : voir [`VERSION`](VERSION) ; nouveautés dans [`CHANGELOG.md`](CHANGELOG.md), aussi affiché dans l'app (Réglages › Application).

## Fonctionnalités

### Écoute

- Lecture FLAC jusqu'en 24 bits / 192 kHz, avec l'édition hi-res d'un album quand elle existe. Sur Android, une qualité pour le Wi-Fi et une autre pour les données mobiles (Basse, Haute en AAC, Lossless, Max), baissée en option quand la batterie est faible.
- Enchaînement sans blanc (gapless) : le titre suivant est préchargé 30 s avant la fin.
- **Mode exclusif WASAPI** sous Windows : la sortie audio est réservée à Seaglass, sans mixage ni rééchantillonnage.
- **Égaliseur paramétrique** (pics, plateaux, préampli), profils nommés, import des profils AutoEQ, courbe de réponse ; sur le bureau, retenu pour chaque sortie audio.
- Chaîne audio affichée en direct : format reçu, décodage, format envoyé à la sortie, avance déjà en mémoire.
- Choix de la sortie audio (réglages propres à chaque sortie sur le bureau), normalisation du volume, pause quand le casque est débranché.
- Minuteur de mise en veille, aussi en fin d'album, de playlist ou de file.
- File d'attente : contexte d'origine, « Lire ensuite », « Ajouter à la file », lecture aléatoire, répétition (tout / un titre), précédent, enregistrement de la file en playlist, bouton « Annuler » après un retrait.
- File, position et volume restaurés au démarrage, reprise de la lecture au lancement en option.
- Radio d'un titre, mixes, radio automatique en fin de file.
- **Transfert de la lecture** entre l'ordinateur et l'app Android (« Reprendre ici », « Continuer sur l'ordinateur »), sur le réseau local ou par le relais, chiffré de bout en bout.
- Android, écran éteint : la puce audio décode l'AAC quand elle sait le faire (offload), et le lecteur sollicite peu le processeur (mesures dans « Par rapport à l'app officielle »).

### Catalogue et bibliothèque

- Accueil personnalisé et réordonnable, recherche pendant la frappe (titres, albums, artistes, playlists) avec historique, pages album, artiste, playlist et mix.
- Bibliothèque : playlists (avec dossiers), albums, artistes, titres likés, mixes, « Plus tard » et « Écoutés dans Seaglass ».
- Playlists : créer, renommer, réordonner, supprimer ; nettoyer les doublons et les titres retirés du catalogue.
- **Import d'une playlist** Spotify, Deezer ou Apple Music en collant son lien, avec un aperçu des titres trouvés.
- Sélection multiple de titres pour les lire, les mettre en file ou les ajouter à une playlist.
- Paroles, crédits cliquables (un nom ouvre les titres où la personne est créditée), vue « En lecture » plein écran.
- Notifications des nouvelles sorties des artistes suivis, en option.
- Masquer le contenu explicite ou la musique générée par IA (badge « IA » sinon).
- Liens `tidal.com` collés ou touchés ouverts directement dans l'app.
- Épingles dans la barre latérale.
- Export de la bibliothèque en JSON et CSV (bureau).

### Statistiques et découverte

- Statistiques d'écoute sur 7 jours, 30 jours, 12 mois ou tout l'historique, et rétrospective du mois et de l'année.
- Playlists intelligentes calculées sur l'appareil : « Mes 50 du mois », « En boucle en ce moment », « Oubliés », « Likés jamais écoutés ».
- Historique d'écoute synchronisé entre l'ordinateur et l'app Android liée.
- Envoi des écoutes à ListenBrainz, en option, gardées hors ligne puis envoyées plus tard.
- **Blind test** à partir d'une playlist, d'un mix, d'un album ou des titres likés : deviner le titre ou l'artiste d'extraits de 15 s, seul ou à plusieurs avec les invités de la Jam.

### Jam

Des invités ajoutent des titres à la file depuis leur téléphone, sans app ni compte : ils scannent un QR code et cherchent dans une page web. Ça marche sur le réseau local, ou en ligne par un relais (sans Wi-Fi commun). La page accepte les liens Spotify, Deezer, Apple Music et YouTube Music. Les invités peuvent voter pour passer un titre, lire les paroles et, si l'hôte le permet, régler le volume ou mettre en pause. L'hôte garde la main : limites par invité, albums permis ou non, ajouts suspendus, invités exclus.

### Interface

- Une vingtaine de thèmes, certains en version claire et sombre : **Verre de mer** (verre dépoli sur la pochette, couleurs accordées à l'album), **Studio**, **Atelier**, **Minuit** (noir pur, pour les écrans OLED), **Acidwave**, **Surréalisme**, **Solarpunk**, **Cyberpunk**, **Neumorphism**, **Terminal**, **Risographie**, **Bauhaus**, **Aéro**, **Alliage**, **Estampe**, **Art déco**, **Cassette**, **Séance**, **Ordinateur de bord**, **Film noir**, **Grand hôtel**, **Désert** et **Générique**.
- Couleur d'accent réglable pour chaque thème, taille de l'interface (80 à 150 %, bureau), animations réduites ou complètes.
- Français et anglais, selon la langue du système ou au choix.
- Bureau : palette de commandes (Ctrl+K) pour aller à une page, lancer une action ou chercher, et raccourcis Espace, Ctrl+,, Échap, Alt+←, F11, Ctrl+« + » / « - » / 0.
- Sauvegarde des réglages en texte, à coller dans Seaglass sur un autre appareil.
- Quelques clins d'œil certains jours, désactivables.

### Intégration au système

- **Windows** : touches média et panneau multimédia (SMTC), icône de la zone de notification, lancement à l'ouverture de session, présence Discord.
- **Linux** : contrôles média par MPRIS (touches média, applet du bureau, `playerctl`, KDE Connect), présence Discord.
- **Bureau** : raccourcis clavier globaux (en option), mini-lecteur au premier plan, **télécommande** depuis le navigateur d'un téléphone sur le même Wi-Fi.
- **Android** : notification et session média (écouteurs, autoradio, montre), widget d'écran d'accueil, épingles en raccourcis de l'icône, mode conduite plein écran à grosses tuiles.
- Mises à jour automatiques signées sur les trois plateformes.
- Journaux et rapports de plantage gardés en local, copiables depuis Réglages › Application.

## Par rapport à l'app officielle

Comparaison faite le 2026-11-18. Les chiffres viennent de mesures sur un Galaxy S21 (#451, méthode et détails dans [`docs/android.md`](docs/android.md) › « Mesures de performances »). Pour l'app officielle, seul ce qui a été vérifié figure ici, et elle évolue : certains points peuvent avoir changé depuis.

### Sur toutes les plateformes

**Mieux dans Seaglass**

- Égaliseur paramétrique, avec import des profils AutoEQ et un réglage par sortie audio sur le bureau. L'app officielle n'a pas d'égaliseur.
- Import d'une playlist Spotify, Deezer ou Apple Music en collant son lien, dans l'app. L'officielle renvoie vers des services tiers.
- Envoi des écoutes à ListenBrainz et présence Discord intégrés. Pour l'officielle, il faut des outils tiers.
- Une vingtaine de thèmes, et la chaîne audio affichée jusqu'au format envoyé à la sortie.

**Aussi dans Seaglass**, sans comparaison vérifiée : la Jam (des invités ajoutent des titres depuis leur téléphone, sans app ni compte), le blind test seul ou à plusieurs, l'accueil réordonnable, les statistiques et la rétrospective, les playlists intelligentes et le nettoyage des doublons, tous calculés sur l'appareil.

**Moins bien dans Seaglass**

- Pas de téléchargement hors ligne : c'est un choix du projet, qui ne fait que du streaming.
- Pas de Dolby Atmos ni de vidéos. Les contenus protégés par DRM ne sont pas lus.
- Pas de TIDAL Connect ni de Chromecast, et pas d'app iOS ni macOS.
- Client non officiel : il repose sur une API privée qui peut changer sans prévenir, et s'installe hors des boutiques d'applications.

### Windows

- **À égalité** : le mode exclusif WASAPI existe dans les deux apps.
- **Mieux** : lecture confiée à mpv, avec la chaîne audio détaillée (format reçu, décodage, format envoyé à la carte son).
- **Aussi dans Seaglass**, sans comparaison vérifiée : la télécommande depuis le navigateur du téléphone, le mini-lecteur, les raccourcis globaux et le transfert de la lecture avec l'app Android.
- **Moins bien** : l'installeur n'est pas signé (SmartScreen demande une confirmation), et le mpv livré exige un processeur AVX2.
- Les performances sur le bureau n'ont pas été mesurées.

### Linux

- **Mieux** : l'app officielle n'existe pas sous Linux, seul le lecteur web est proposé. Seaglass est une app native, avec les contrôles média du bureau (MPRIS) et la lecture par mpv.
- **Moins bien** : pas de mode exclusif, et mpv doit être installé par le système.

### Android

Mesuré sur le S21, en % d'un cœur du processeur (moins = mieux) :

| Scénario | Seaglass 0.5.90 | App officielle 2.219.0 |
|---|---|---|
| Écran éteint, lecture lossless (30 min) | **8,5 %** | 20,5 % |
| Écran éteint, qualité Haute (AAC), Dolby Atmos coupé | **1,6 à 2,5 %** | non mesuré |
| « En lecture » ouvert (5 min) | **9,8 %** | 81 % |
| Accueil pendant la lecture | **5,7 %** | 12 % |
| Défilement rapide d'une playlist de 489 titres | **42 %** (1,5 % d'images en retard) | 63 % (**0,2 %** d'images en retard) |
| Accueil affiché après un démarrage à froid | ~1,6 à 2 s | ~1,7 à 2,4 s |

Même playlist, en FLAC 16 bits / 44,1 kHz dans les deux apps : Seaglass la lisait en streaming, l'app officielle depuis ses fichiers téléchargés. Téléphone relié en USB ; les consommations sont donc estimées par Android, pas mesurées sur la batterie.

- **Mieux** : moins de batterie, écran éteint comme allumé. Une qualité différente en Wi-Fi et en données mobiles, baissée en option quand la batterie est faible. Sur les données mobiles, la puce audio décode l'AAC (offload), si Dolby Atmos est coupé dans les réglages du téléphone.
- **Aussi dans Seaglass**, sans comparaison vérifiée : le mode conduite à grosses tuiles, le widget et les épingles en raccourcis de l'icône.
- **Moins bien** : pas d'Android Auto (le mode conduite le remplace en partie). En qualité Max, le 24 bits est ramené à 16 bits, et il n'y a pas de sortie bit-perfect vers un DAC USB. Le défilement est un peu moins régulier. L'APK s'installe hors du Play Store.

## Installation

Les versions publiées par l'auteur se téléchargent sur **https://updates.bebanana.ch/seaglass/** ; pour construire et distribuer les siennes, sans ce serveur, voir « Construire et héberger soi-même ». Ensuite, l'app se met à jour toute seule : quand une nouvelle version est prête, elle propose « Installer et redémarrer » ou « Plus tard ».

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

   Adresses des services en ligne, facultatives : voir « Construire et héberger soi-même ».

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

## Construire et héberger soi-même

Rien ne dépend du serveur de l'auteur : aucune adresse ni clé n'est écrite dans le code ou les scripts. Elles sont lues à la compilation, dans l'environnement ou dans `.env` (modèle complet : [`.env.example`](.env.example)). Sans elles, l'app marche sans le service correspondant.

À fournir soi-même :

| Quoi | Requis pour | Où |
|---|---|---|
| `TIDAL_CLIENT_ID` / `TIDAL_CLIENT_SECRET` | se connecter | `.env` (voir [`docs/tidal.md`](docs/tidal.md)) |
| Clé de l'updater (`tauri signer generate -w ~/.tauri/seaglass.key`) | publier des mises à jour du bureau | hors du dépôt ; la clé publique est compilée depuis le `.pub` |
| `SEAGLASS_UPDATE_BASE_URL` + un dossier public en HTTPS | publier des versions | `.env` ; `SEAGLASS_RELEASE_DEST` pour l'envoi |
| Keystore Android | publier l'APK | voir [`docs/android.md`](docs/android.md) |
| `SEAGLASS_RELAY_BASE_URL` / `SEAGLASS_RELAY_KEY` + un relais | Jam en ligne, transfert de lecture | `.env`, et [`relay/`](relay/README.md) |
| `DISCORD_CLIENT_ID` | présence Discord | `.env` |

```
# Relais de la Jam en ligne et du transfert de lecture (facultatif)
SEAGLASS_RELAY_BASE_URL=https://jam.example.org
SEAGLASS_RELAY_KEY=...
# Dossier des mises à jour (facultatif ; posé par les scripts de release)
SEAGLASS_UPDATE_BASE_URL=https://updates.example.org/seaglass
```

- **Relais** : chacun peut héberger le sien (dossier [`relay/`](relay/README.md), même clé `SEAGLASS_RELAY_KEY` que l'app). Dans l'app, Réglages › Application › Relais permet aussi d'en choisir un autre, avec sa clé, ou de le couper. Sans relais, la Jam reste possible sur le réseau local (ordinateur), et l'interrupteur « Jam » la retire de l'app.
- **Mises à jour** : l'app n'installe que des versions signées par la clé dont la partie publique est compilée (`SEAGLASS_UPDATER_PUBKEY`, que les scripts de release lisent dans `<clé privée>.pub` ; certificat de l'APK sur Android). Pour publier ses propres versions, il faut donc sa propre clé, son propre dossier, et les compiler dans son build. Sans adresse ou sans clé, l'app ne propose aucune mise à jour ; avec, la vérification automatique se coupe dans Réglages › Application.

## Publier une version

1. Monter `VERSION` (et `src-tauri/Cargo.toml`), ajouter l'entrée de `CHANGELOG.md`, commiter.
2. `pnpm release:all` : construit l'installeur Windows, l'APK et l'AppImage (dans WSL), signés, avec les manifestes de mise à jour, sans rien publier. `pnpm release` ne construit que l'installeur Windows.
3. `pnpm release:all -Publish` : publie dans le dossier des mises à jour, pose le tag `v<version>` et crée la release Gitea.

Détails, variables d'environnement et clés dans [`docs/release.md`](docs/release.md). Les clés privées de signature (updater et APK) doivent être sauvegardées : sans elles, les versions installées refuseront toute mise à jour.

## Licence

[GPL-3.0](LICENSE). mpv est lancé comme programme séparé et piloté par IPC, il n'est pas lié au code de Seaglass ; sa notice de licence est livrée avec l'installeur (`src-tauri/licenses/mpv.txt`).
