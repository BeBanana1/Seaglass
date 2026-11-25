# Jam relay

Makes a Seaglass Jam reachable from the Internet (#25, step 3a): guests do not need to be on the same Wi-Fi as the host, everyone stays on their mobile data.

- The host's app connects over WebSocket to `/host` (outgoing connection: no port to open on the host's side, works behind a mobile network).
- Guests open `https://<relay>/j/<id>/#<code>` (QR code). The page is the local Jam's (`src-tauri/src/jam/page/`, embedded at build time).
- Device tunnels (#430): a PC connects to `/device` (same key) and its linked phone calls it through `POST /d/<id>`, for playback handoff and listening history. Bodies are end-to-end encrypted by both devices: the relay cannot read them. Details in `docs/jam.md`.
- Their calls (`join`, `search`, `add`, `state`) are forwarded to the host, which alone talks to TIDAL and to the player. The relay serves the state stream (`api/events`) itself, from the last state pushed by the host.
- The relay only sees track titles, first names and the Jam code: never a TIDAL token, stream URL or audio.

Protocol: `src/protocol.rs`.

## Security and limits

- **Build key** (`SEAGLASS_RELAY_KEY`, `Authorization: Bearer …` header on `/host`): the same value in the app's `.env`. It keeps out bots scanning the Internet. It is not a secret: anyone can extract it from a distributed app. Protection comes from the limits below.
- 128-bit Jam identifier in the link, access code checked by the host (and by the relay for the state stream, 10 failed attempts per address).
- At most 20 open Jams (`RELAY_MAX_JAMS`), of which 3 for the same host address (`RELAY_MAX_JAMS_PER_ADDRESS`), 10 creations per address per hour, 12 h lifetime, 30 min for the host to come back after a drop or a screen-off pause (same address, same QR code). Resuming uses the Jam's secret, even if the relay still believes the old connection is open: a phone that switches networks comes back before the relay has noticed the drop.
- 300 calls per minute per address (several guests on mobile data often share an address), 10 wrong codes per quarter hour per address for a Jam, 2 KiB bodies, 64 open pages per Jam, 256 KiB host messages, host answer awaited for 15 s.
- "Address" means an IPv4 address or an IPv6 /64 network. A body is only read once its call has passed the rate limit and found its Jam or device; phone packets in memory share a 16 MiB budget, and a PC's connection is only read while its phone awaits an answer, 4 at a time (the container has 128 MiB).
- Read-only container, no capabilities, non-root user, 128 MB of memory.

## Deployment (server, Docker, behind Caddy)

Example with `jam.example.org`: replace it with your own domain name.

1. **DNS**: `jam.example.org` to the server (A or CNAME record).
2. **`relay/.env`** (copy of `.env.example`):
   - `RELAY_PUBLIC_URL=https://jam.example.org` (required), the same address as `SEAGLASS_RELAY_BASE_URL` in the app's `.env`;
   - `SEAGLASS_RELAY_KEY`: `openssl rand -hex 32`, the same value in the app's `.env`.
3. **Port**: Caddy runs with `network_mode: host`, it reaches the relay at `127.0.0.1:47880` (`RELAY_PORT` in `relay/.env` if that port is taken: `ss -ltn | grep 47880`). The port is only published on loopback: nobody bypasses Caddy.
4. **Build and run**, from a clone of the repository:

   ```sh
   cd relay
   docker compose up -d --build
   ```

5. **Caddy**: add to the Caddyfile, then reload Caddy.

   ```caddyfile
   jam.example.org {
       reverse_proxy 127.0.0.1:47880
   }
   ```

   Caddy handles the WebSocket and the state stream (`text/event-stream`, sent unbuffered) on its own, and sets `X-Forwarded-For`, which the relay reads (`RELAY_TRUST_PROXY=1` in the image).

6. **Check**: `curl http://127.0.0.1:47880/healthz` then `curl https://jam.example.org/healthz` answer `ok`. Logs: `docker compose logs -f relay`.

Update: `git pull` then `docker compose up -d --build`.

## Development

```sh
cd relay
cargo test
SEAGLASS_RELAY_KEY=dev RELAY_PUBLIC_URL=http://127.0.0.1:8080 RELAY_BIND=127.0.0.1:8080 cargo run
```

Crate separate from the app (no workspace), tested by CI. A change to the relay alone does not change the app version (`scripts/check-version.sh` excludes it).
