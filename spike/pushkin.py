"""Spike #35: TIDAL's one-stream-per-account signal ("Pushkin"), as the official web SDK uses it.

Read in tidal-music/tidal-sdk-web, packages/player/src/internal/services/pushkin.ts
(handlers/load.ts calls `Pushkin.ensure()`, handlers/play.ts sends `USER_ACTION`):

- `POST https://api.tidal.com/v1/rt/connect` (Bearer token) answers `{"url": <WebSocket URL>}`.
- Client -> server: `{"type": "USER_ACTION", "payload": {"startedAt": <ms>}}` on an explicit Play.
- Server -> client: `PRIVILEGED_SESSION_NOTIFICATION` (`payload {clientDisplayName, sessionId,
  endsAt, updatedAt}`): the SDK pauses its player. `RECONNECT`: fetch a new URL, reconnect.

Usage:

    python -I spike/pushkin.py listen [--user-action] [--duration <s>]
        Connects and prints every message from the server, with the time. Enter sends a
        USER_ACTION (what an explicit Play sends); --user-action sends one right away.
        Ctrl+C quits, or --duration (also lets it run without a console).

    python -I spike/pushkin.py play <track id or URL> [--no-pushkin]
        Connects, sends a USER_ACTION, then plays the track in mpv. On a
        PRIVILEGED_SESSION_NOTIFICATION, pauses mpv as the SDK does. Enter in this console
        is an explicit Play: a USER_ACTION, then mpv resumes. --no-pushkin plays without
        connecting, to see whether the server itself cuts a second stream.

What to check (issue #35, step 2):
1. Does `rt/connect` accept the PKCE client's token?
2. Does a USER_ACTION from here pause an official app, and does Play on an official app
   send us a PRIVILEGED_SESSION_NOTIFICATION?
3. What does `clientDisplayName` hold?
4. With two clients streaming and no Pushkin (`play --no-pushkin`), does the server
   refuse or cut anything?

`startedAt` uses the local clock (the SDK uses a server-synced one, `trueTime`): the clock
offset estimated from the HTTP `Date` header is printed, to rule it out if a takeover is
ignored.

Throwaway script. Nothing is written to disk (no audio, no token). The token and the
WebSocket URL (it may carry a credential) are never printed.
"""

from __future__ import annotations

import argparse
import base64
import json
import subprocess
import sys
import threading
import time
import uuid
from email.utils import parsedate_to_datetime
from pathlib import Path
from urllib.parse import urlsplit

import requests
import websocket  # websocket-client

sys.path.insert(0, str(Path(__file__).resolve().parent))
import play  # noqa: E402 - spike helpers (session, mpv, DRM check)
import playlog  # noqa: E402 - stream request with a streaming session id, DRM-checked URL

RT_CONNECT_URL = "https://api.tidal.com/v1/rt/connect"
PIPE = r"\\.\pipe\seaglass-pushkin-spike"
# Measured: the server closes a connection after 5 s without traffic (no close frame,
# with or without an Origin header). Client pings, answered by pongs, keep it open.
KEEPALIVE_S = 2


def now_ms() -> int:
    return int(time.time() * 1000)


def log(message: str) -> None:
    t = time.time()
    print(f"[{time.strftime('%H:%M:%S', time.localtime(t))}.{int(t * 1000) % 1000:03d}] {message}", flush=True)


def jwt_claims(token: str) -> dict:
    """Unverified claims of the access token, to compare `sid` with a notification's."""
    try:
        part = token.split(".")[1]
        return json.loads(base64.urlsafe_b64decode(part + "=" * (-len(part) % 4)))
    except (IndexError, ValueError):
        return {}


# --- Pushkin -------------------------------------------------------------------------


class Pushkin:
    """The SDK's Pushkin client: one WebSocket, reconnected on RECONNECT or close."""

    def __init__(self, session, on_revoked=None) -> None:
        self.session = session
        self.on_revoked = on_revoked
        self.ws: websocket.WebSocket | None = None
        self.secrets: set[str] = set()
        self.closed = False
        self._send_lock = threading.Lock()
        claims = jwt_claims(session.access_token)
        self.own_sid = str(claims["sid"]) if "sid" in claims else None

    def redact(self, text: str) -> str:
        for secret in self.secrets | {self.session.access_token}:
            if secret:
                text = text.replace(secret, "<hidden>")
        return text

    def fetch_url(self) -> str:
        sent = time.time()
        resp = requests.post(
            RT_CONNECT_URL,
            headers={
                "Authorization": f"Bearer {self.session.access_token}",
                "Content-Type": "application/json",
            },
            timeout=15,
        )
        received = time.time()
        log(f"POST rt/connect: HTTP {resp.status_code}")
        if resp.status_code != 200:
            print(self.redact(resp.text[:1000]))
            sys.exit("rt/connect refused: stopping.")
        if resp.headers.get("Date"):
            server = parsedate_to_datetime(resp.headers["Date"]).timestamp()
            log(f"Clock offset (server - local), 1 s resolution: {server - (sent + received) / 2:+.1f} s")
        data = resp.json()
        url = data.get("url")
        if not url:
            sys.exit(f"No url in the answer (keys: {sorted(data)}).")
        self.secrets.add(url)
        parts = urlsplit(url)
        others = sorted(k for k in data if k != "url")
        log(
            f"WebSocket at {parts.scheme}://{parts.netloc} "
            f"(path {len(parts.path)} chars, query {len(parts.query)} chars, hidden)"
            + (f"; other keys: {others}" if others else "")
        )
        return url

    def connect(self) -> None:
        url = self.fetch_url()
        try:
            self.ws = websocket.create_connection(url, timeout=15)
        except websocket.WebSocketBadStatusException as exc:
            sys.exit(f"WebSocket handshake refused: HTTP {exc.status_code}")
        self.ws.settimeout(None)
        log("WebSocket open")

    def reconnect(self) -> None:
        delay = 2
        while not self.closed:
            if self.ws:
                try:
                    self.ws.close()
                except OSError:
                    pass
            try:
                self.connect()
                return
            except (OSError, websocket.WebSocketException, requests.RequestException) as exc:
                log(f"Reconnection failed ({type(exc).__name__}), retrying in {delay} s")
                time.sleep(delay)
                delay = min(delay * 2, 30)

    def user_action(self) -> None:
        message = {"type": "USER_ACTION", "payload": {"startedAt": now_ms()}}
        try:
            with self._send_lock:
                if self.ws is None:
                    raise websocket.WebSocketConnectionClosedException("not connected")
                self.ws.send(json.dumps(message))
        except (OSError, websocket.WebSocketException) as exc:
            log(f"USER_ACTION not sent ({type(exc).__name__})")
            return
        log(f"Sent {json.dumps(message)}")

    def keepalive(self) -> None:
        """The server drops a connection idle for 5 s (no close frame): ping every 2 s."""
        while not self.closed:
            time.sleep(KEEPALIVE_S)
            try:
                with self._send_lock:
                    if self.ws is not None and self.ws.connected:
                        self.ws.ping()
            except (OSError, websocket.WebSocketException):
                pass  # the receive loop sees the drop and reconnects

    def run(self) -> None:
        """Receive loop: prints and handles every message, reconnects when the socket drops."""
        while not self.closed:
            try:
                raw = self.ws.recv() if self.ws else ""
            except (OSError, websocket.WebSocketException) as exc:
                if self.closed:
                    return
                log(f"WebSocket dropped ({type(exc).__name__}), reconnecting")
                self.reconnect()
                continue
            if not raw:
                if self.closed:
                    return
                log("WebSocket closed by the server, reconnecting")
                self.reconnect()
                continue
            self.handle(raw if isinstance(raw, str) else raw.decode("utf-8", errors="replace"))

    def handle(self, raw: str) -> None:
        log(f"Received: {self.redact(raw)}")
        try:
            event = json.loads(raw)
        except ValueError:
            return
        kind = event.get("type")
        if kind == "PRIVILEGED_SESSION_NOTIFICATION":
            payload = event.get("payload") or {}
            name = payload.get("clientDisplayName")
            own = None if self.own_sid is None else str(payload.get("sessionId")) == self.own_sid
            log(
                f"-> playback taken by {name!r}; its sessionId "
                + ("is unknown here (no sid claim)" if own is None else "is OURS" if own else "is another session's")
            )
            if self.on_revoked:
                self.on_revoked(name)
        elif kind == "RECONNECT":
            log("-> RECONNECT asked")
            self.reconnect()
        else:
            log(f"-> unhandled type {kind!r}")

    def close(self) -> None:
        self.closed = True
        if self.ws:
            try:
                self.ws.close()
            except OSError:
                pass


def start(session, on_revoked=None) -> Pushkin:
    pushkin = Pushkin(session, on_revoked)
    log(f"Token sid claim: {'present' if pushkin.own_sid else 'absent'}")
    pushkin.connect()
    threading.Thread(target=pushkin.run, daemon=True).start()
    threading.Thread(target=pushkin.keepalive, daemon=True).start()
    return pushkin


# --- mpv -----------------------------------------------------------------------------


def mpv_set_pause(paused: bool) -> None:
    command = {"command": ["set_property", "pause", paused]}
    try:
        with open(PIPE, "r+b", buffering=0) as pipe:  # noqa: PTH123
            pipe.write((json.dumps(command) + "\n").encode())
    except OSError as exc:
        log(f"mpv not reachable ({type(exc).__name__})")
        return
    log("mpv paused" if paused else "mpv resumed")


# --- Commands ------------------------------------------------------------------------


def cmd_listen(args: argparse.Namespace) -> None:
    pushkin = start(play.require_session())
    if args.user_action:
        pushkin.user_action()
    log("Listening. Enter = USER_ACTION, Ctrl+C = quit.")
    deadline = time.time() + args.duration if args.duration else None
    try:
        if sys.stdin and sys.stdin.isatty():
            for _ in sys.stdin:
                pushkin.user_action()
        # Without a console (or after EOF): keep listening until the deadline or Ctrl+C.
        while deadline is None or time.time() < deadline:
            time.sleep(0.5)
    except KeyboardInterrupt:
        pass
    log("Done.")
    pushkin.close()


def cmd_play(args: argparse.Namespace) -> None:
    track_id = play.parse_track_id(args.track)
    session = play.require_session()
    track = session.track(track_id)

    pushkin = None
    if not args.no_pushkin:
        pushkin = start(session, on_revoked=lambda _name: mpv_set_pause(True))
        pushkin.user_action()  # play() sends it before the player starts

    info = playlog.playback_info(session, track_id, str(uuid.uuid4()))
    log(f"{track.artist.name} – {track.full_name}: {info.get('audioQuality')} {info.get('assetPresentation')}")
    url, server = playlog.stream_url(info)
    mpv = [
        play.find_mpv(),
        "--no-config",
        "--no-video",
        "--ytdl=no",
        "--cache=yes",
        "--cache-on-disk=no",  # principle 1: nothing on disk
        "--idle=no",
        "--msg-level=all=error",
        f"--input-ipc-server={PIPE}",
        f"--force-media-title={track.artist.name} – {track.full_name}",
        url,
    ]
    proc = subprocess.Popen(mpv)
    log("Playing in mpv (q in mpv to stop)." + (" Enter here = explicit Play." if pushkin else ""))

    def explicit_play() -> None:
        for _ in sys.stdin:
            if pushkin:
                pushkin.user_action()
            mpv_set_pause(False)

    threading.Thread(target=explicit_play, daemon=True).start()
    try:
        code = proc.wait()
    except KeyboardInterrupt:
        proc.terminate()
        code = proc.wait()
    log(f"mpv exited with code {code}")
    if server:
        server.shutdown()
    if pushkin:
        pushkin.close()


def main() -> None:
    play.load_env_file()
    parser = argparse.ArgumentParser(description="Spike #35: TIDAL's Pushkin service")
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("listen", help="connect and print every message")
    p.add_argument("--user-action", action="store_true", help="send a USER_ACTION right away")
    p.add_argument("--duration", type=float, help="stop after this many seconds")
    p.set_defaults(func=cmd_listen)

    p = sub.add_parser("play", help="play a track, pausing when another device takes over")
    p.add_argument("track")
    p.add_argument("--no-pushkin", action="store_true", help="play without connecting to Pushkin")
    p.set_defaults(func=cmd_play)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
