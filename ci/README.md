# Runner CI de Seaglass

Runner Gitea Actions dédié au dépôt, sur le serveur, avec sa propre image de jobs
(Node, pnpm, Rust et les bibliothèques de Tauri déjà installés) et un cache qui
fonctionne. Le workflow (`.gitea/workflows/ci.yml`) cible le label `seaglass`.

## Installation

1. Copier ce dossier sur le serveur, par exemple dans `~/seaglass-runner/` :

   ```powershell
   scp -r ci <serveur>:~/seaglass-runner
   ```

2. Sur le serveur, créer le `.env` à partir de `.env.example`. Le jeton vient du
   dépôt : **Paramètres → Actions → Runners → Créer un runner**. Le prendre au
   niveau du dépôt (pas de l'instance) : le runner monte le socket Docker, il ne
   doit servir qu'à Seaglass.

3. Construire l'image des jobs, puis lancer le runner :

   ```sh
   docker compose --profile build build
   docker compose up -d
   ```

4. Vérifier que le runner `seaglass` apparaît en ligne dans Gitea, avec le label
   `seaglass`.

## Mettre à jour l'image

Nouvelle base Debian, nouvelle majeure de pnpm, ou Rust trop ancien (le workflow
le met à jour à chaque run, mais ça ralentit) : changer le tag partout, puis
reconstruire. Pareil pour `cargo-audit` (#263) : une image construite avant son
ajout fonctionne, le workflow le compile lui-même, mais le premier run est plus long.

1. `SEAGLASS_CI_TAG` dans `.env` et le label de `config.yaml` (même valeur).
2. `docker compose --profile build build`, puis `docker compose up -d` (le runner
   relit sa config au redémarrage).
3. Supprimer l'ancienne image : `docker image rm seaglass-ci:<ancien tag>`.

## Dépannage

- **Jobs en attente indéfiniment** : le runner n'est pas en ligne, ou son label ne
  correspond pas à `runs-on: seaglass`.
- **`ETIMEDOUT` sur le cache** : les jobs ne sont pas sur le réseau `seaglass-ci`
  (`container.network` dans `config.yaml`). Le cache est facultatif : le run passe
  quand même, mais perd 5 min par action de cache.
- **`pull access denied` pour `seaglass-ci`** : l'image n'a pas été construite, ou
  son tag diffère du label.
- **Clé de config inconnue** après une mise à jour d'act_runner : comparer avec
  `docker run --rm --entrypoint act_runner docker.io/gitea/act_runner:<version> generate-config`.
