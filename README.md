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
- Trois thèmes : **Verre de mer** (verre dépoli sur la pochette, couleurs accordées à l'album), **Studio** (console sombre et dense) et **Atelier** (clair, éditorial).
- Vue « En lecture » plein écran ; raccourcis : Espace, Ctrl+K, Échap, Alt+←.

## Prérequis

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

L'architecture, les conventions et la feuille de route sont décrites dans [`CLAUDE.md`](CLAUDE.md). Le dossier `spike/` contient le script Python de la phase 0 qui a validé les formats de flux.

## Licence

[GPL-3.0](LICENSE). mpv est lancé comme programme séparé et piloté par IPC, il n'est pas lié au code de Seaglass.
