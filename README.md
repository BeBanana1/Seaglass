# Seaglass

Client desktop de streaming musical pour Windows, avec une interface personnalisée et une lecture lossless / bit-perfect confiée à [mpv](https://mpv.io).

> **Client non officiel, non affilié à TIDAL.** Un abonnement valide est nécessaire. Streaming uniquement : aucun flux audio n'est téléchargé ni enregistré sur disque, et les flux protégés par DRM ne sont pas lus.

Version actuelle : voir [`VERSION`](VERSION).

## Fonctionnalités

- Connexion au compte (OAuth PKCE) ; le jeton est rangé dans le Gestionnaire d'identifiants Windows.
- Accueil personnalisé, recherche (titres, albums, artistes, playlists), pages album, artiste et playlist.
- Bibliothèque : playlists, albums, artistes et titres favoris.
- Lecture FLAC jusqu'en 24 bits.
- File d'attente : contexte d'origine, « Lire ensuite », « Ajouter à la file », lecture aléatoire, répétition (tout / un titre), précédent.
- Enchaînement sans blanc (gapless) : le titre suivant est préchargé 30 s avant la fin.
- File et position restaurées au démarrage.
- Chaîne audio affichée en direct : format reçu, décodage, format envoyé à la carte son.
- Touches média du clavier et panneau multimédia de Windows (SMTC).
- Douze thèmes : **Verre de mer** (verre dépoli sur la pochette, couleurs accordées à l'album), **Studio** (console sombre et dense), **Atelier** (clair, éditorial), **Acidwave** (néons acides et chrome, qui pulse pendant la lecture), **Surréalisme** (paysage vectoriel onirique teinté par l'album), **Solarpunk** (futur végétal et lumineux, pochettes en arche), **Cyberpunk** (interface de visée dans une ville néon, angles coupés), **Neumorphism** (reliefs doux ton sur ton, en clair et en sombre), **Terminal** (écran cathodique monochrome dont la couleur d'accent est le phosphore), **Risographie** (fanzine deux encres, papier grainé, pochettes en bichromie) et **Bauhaus** (affiche géométrique, grille et couleurs primaires).
- Page Réglages (Ctrl+,) : thème, taille du cache de lecture (en mémoire), lancement à l'ouverture de session.
- Vue « En lecture » plein écran ; raccourcis : Espace, Ctrl+K, Ctrl+,, Échap, Alt+←.

## Installation

Télécharger la dernière version sur **https://updates.bebanana.ch/seaglass/** et lancer l'installeur : l'installation se fait pour l'utilisateur courant, sans droits administrateur, et mpv est inclus. L'installeur n'a pas de certificat de signature Windows : SmartScreen demande « Informations complémentaires » puis « Exécuter quand même ». Ensuite, l'app se met à jour toute seule : quand une nouvelle version est prête, elle propose « Installer et redémarrer » ou « Plus tard » (installation à la fermeture). Processeur compatible AVX2 requis par le build de mpv livré.

## Prérequis (développement)

- Windows 10 ou 11 (WebView2 installé)
- [mpv](https://mpv.io/installation/) dans le `PATH`, ou son chemin dans la variable `SEAGLASS_MPV_PATH`
- [Rust](https://rustup.rs) stable et les Visual Studio Build Tools (C++)
- [Node.js](https://nodejs.org) LTS et [pnpm](https://pnpm.io)

## Démarrage

1. Créer un fichier `.env` à la racine (il est ignoré par git) :

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
```

Aperçu de l'interface sans Tauri, sur des données factices : `pnpm dev`, puis `http://127.0.0.1:1420/?theme=studio&route=album` (voir CLAUDE.md, « Prévisualisation sans Tauri »).

L'architecture, les conventions et la feuille de route sont décrites dans [`CLAUDE.md`](CLAUDE.md). Les bugs et demandes sont suivis dans les issues du dépôt Gitea ; chaque correction passe par une branche `issue-<n>-…` et une PR qui ferme l'issue. Le dossier `spike/` contient le script Python de la phase 0 qui a validé les formats de flux.

## Publier une version

1. Monter `VERSION` (et `src-tauri/Cargo.toml`), commiter.
2. `pnpm release` : construit l'installeur signé, `release/latest.json` et la page de téléchargement `release/index.html`, sans rien publier.
3. `pnpm release -Publish` avec `SEAGLASS_RELEASE_DEST` défini : publie dans le dossier des mises à jour et pose le tag `v<version>`.

La clé privée de signature (`%USERPROFILE%\.tauri\seaglass.key`) doit être sauvegardée : sans elle, les versions installées refuseront toute mise à jour.

## Licence

[GPL-3.0](LICENSE). mpv est lancé comme programme séparé et piloté par IPC, il n'est pas lié au code de Seaglass ; sa notice de licence est livrée avec l'installeur (`src-tauri/licenses/mpv.txt`).
