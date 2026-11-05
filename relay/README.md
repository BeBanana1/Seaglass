# Relais de la Jam

Rend une Jam Seaglass joignable depuis Internet (#25, étape 3a) : les invités n'ont pas besoin d'être sur le même Wi-Fi que l'hôte, chacun reste sur ses données mobiles.

- L'app de l'hôte se connecte en WebSocket à `/host` (connexion sortante : aucun port à ouvrir chez l'hôte, fonctionne derrière le réseau mobile).
- Les invités ouvrent `https://jam.bebanana.ch/j/<id>/#<code>` (QR code). La page est celle de la Jam locale (`src-tauri/src/jam/page/`, embarquée à la compilation).
- Leurs appels (`join`, `search`, `add`, `state`) sont transmis à l'hôte, qui seul parle à TIDAL et au lecteur. Le relais sert lui-même le flux d'état (`api/events`) à partir du dernier état poussé par l'hôte.
- Le relais ne voit passer que des titres, des prénoms et le code de la Jam : jamais de jeton TIDAL, d'URL de flux ni d'audio.

Protocole : `src/protocol.rs`.

## Sécurité et limites

- **Clé de build** (`SEAGLASS_RELAY_KEY`, en-tête `Authorization: Bearer …` de `/host`) : la même valeur dans le `.env` de l'app. Elle écarte les robots qui scannent Internet. Ce n'est pas un secret : n'importe qui peut la tirer d'une app distribuée. La protection vient des limites ci-dessous.
- Identifiant de Jam de 128 bits dans le lien, code d'accès vérifié par l'hôte (et par le relais pour le flux d'état, 10 essais ratés par adresse).
- 20 Jams ouvertes au plus (`RELAY_MAX_JAMS`), 10 créations par adresse et par heure, 12 h de vie, 5 min pour que l'hôte revienne après une coupure (même adresse, même QR code).
- 120 appels par minute et par adresse, corps de 2 Kio, 64 pages ouvertes par Jam, messages de l'hôte de 256 Kio, réponse de l'hôte attendue 15 s.
- Conteneur en lecture seule, sans capacités, utilisateur non root, 128 Mo de mémoire.

## Déploiement (serveur, Docker, derrière Caddy)

1. **DNS** : `jam.bebanana.ch` en CNAME vers `bebanana.internet-box.ch` (comme `updates`).
2. **Clé** : `openssl rand -hex 32`. La mettre dans `relay/.env` (copie de `.env.example`) et dans le `.env` de l'app (`SEAGLASS_RELAY_KEY`, utilisé à partir de l'étape 3b).
3. **Réseau** : `CADDY_NETWORK` dans `relay/.env` = réseau Docker du conteneur Caddy (`docker network ls`). Le relais n'expose aucun port : seul Caddy le joint.
4. **Construire et lancer**, depuis un clone du dépôt :

   ```sh
   cd relay
   docker compose up -d --build
   ```

5. **Caddy** : ajouter au Caddyfile, puis recharger Caddy.

   ```caddyfile
   jam.bebanana.ch {
       reverse_proxy seaglass-relay:8080
   }
   ```

   Caddy gère seul le WebSocket et le flux d'état (`text/event-stream`, envoyé sans tampon), et pose `X-Forwarded-For`, que le relais lit (`RELAY_TRUST_PROXY=1` dans l'image).

6. **Vérifier** : `curl https://jam.bebanana.ch/healthz` répond `ok`. Journaux : `docker compose logs -f relay`.

Mise à jour : `git pull` puis `docker compose up -d --build`.

## Développement

```sh
cd relay
cargo test
SEAGLASS_RELAY_KEY=dev RELAY_PUBLIC_URL=http://127.0.0.1:8080 RELAY_BIND=127.0.0.1:8080 cargo run
```

Crate séparée de l'app (pas de workspace), testée par la CI. Une modification du relais seul ne change pas la version de l'app (`scripts/check-version.sh` l'exclut).
