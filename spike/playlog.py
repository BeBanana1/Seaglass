"""Spike #34: report a real listen to TIDAL's play log, the way the official SDKs do.

Format read in tidal-music/tidal-sdk-web (player/src/internal/event-tracking/play-log/,
event-producer/) and tidal-sdk-android (eventproducer/, player/events/).

Two separate steps, so that what is sent can be read before sending it:

    python -I spike/playlog.py listen <track id or URL> <event.json>
        Requests the stream with an `x-tidal-streamingsessionid` header, plays it for
        real in mpv, then writes the `playback_session` event to <event.json>.
        The file holds no token.

    python -I spike/playlog.py send <event.json> [--dry-run]
        Prints the event, then posts it to ec.tidal.com/api/event-batch. The token is
        only added in memory, at send time, and is never printed.

Throwaway script. Nothing is written to disk but the event file (no audio, no token).
"""

from __future__ import annotations

import argparse
import base64
import json
import re
import subprocess
import sys
import threading
import time
import uuid
from pathlib import Path
from urllib.parse import urlencode

import requests

sys.path.insert(0, str(Path(__file__).resolve().parent))
import play  # noqa: E402 - spike helpers (session, mpv, DRM check)

EVENT_BATCH_URL = "https://ec.tidal.com/api/event-batch"
VERSION_FILE = Path(__file__).resolve().parent.parent / "VERSION"
PIPE = r"\\.\pipe\seaglass-playlog-spike"


def now_ms() -> int:
    return int(time.time() * 1000)


# --- listen --------------------------------------------------------------------------


def playback_info(session, track_id: int, streaming_session_id: str) -> dict:
    resp = session.request.request(
        "GET",
        f"tracks/{track_id}/playbackinfopostpaywall",
        params={
            "audioquality": "LOSSLESS",
            "playbackmode": "STREAM",
            "assetpresentation": "FULL",
        },
        headers={"x-tidal-streamingsessionid": streaming_session_id},
    )
    resp.raise_for_status()
    return resp.json()


def stream_url(info: dict) -> tuple[str, object]:
    """Playable URL for mpv and the local server to stop (DASH), after the DRM check."""
    raw = base64.b64decode(info["manifest"]).decode("utf-8", errors="replace")
    mime = info.get("manifestMimeType", "")
    if any(m.lower() in raw.lower() for m in play.DRM_MARKERS):
        sys.exit("Protected stream: not played.")
    if mime.startswith("application/dash+xml"):
        server, url = play.serve_manifest(raw)
        return url, server
    bts = json.loads(raw)
    if (bts.get("encryptionType") or "NONE") != "NONE":
        sys.exit("Protected stream: not played.")
    return bts["urls"][0], None


class MpvWatcher:
    """Polls mpv over JSON IPC: first heard position, pauses, last position."""

    def __init__(self) -> None:
        self.start_ts: int | None = None
        self.last_pos = 0.0
        self.last_ts: int | None = None
        self.duration: float | None = None
        self.actions: list[dict] = []
        self._paused = False

    def run(self, proc: subprocess.Popen) -> None:
        pipe = None
        for _ in range(100):
            try:
                pipe = open(PIPE, "r+b", buffering=0)  # noqa: SIM115
                break
            except OSError:
                time.sleep(0.1)
        if pipe is None:
            return
        rid = 0
        while proc.poll() is None:
            rid += 1
            try:
                pos = self._get(pipe, "time-pos", rid)
                paused = self._get(pipe, "pause", rid + 1000000)
                if self.duration is None:
                    self.duration = self._get(pipe, "duration", rid + 2000000)
            except OSError:
                break
            ts = now_ms()
            if isinstance(pos, (int, float)):
                if self.start_ts is None and pos >= 0:
                    self.start_ts = ts
                if paused is True and not self._paused:
                    self.actions.append(action("PLAYBACK_STOP", pos, ts))
                elif paused is False and self._paused:
                    self.actions.append(action("PLAYBACK_START", pos, ts))
                if isinstance(paused, bool):
                    self._paused = paused
                self.last_pos = float(pos)
                self.last_ts = ts
            time.sleep(0.25)

    @staticmethod
    def _get(pipe, prop: str, rid: int):
        pipe.write((json.dumps({"command": ["get_property", prop], "request_id": rid}) + "\n").encode())
        while True:
            line = pipe.readline()
            if not line:
                raise OSError("pipe closed")
            msg = json.loads(line)
            if msg.get("request_id") == rid:
                return msg.get("data")


def action(kind: str, pos: float, ts: int) -> dict:
    return {"actionType": kind, "assetPosition": round(float(pos), 3), "timestamp": ts}


def cmd_listen(args: argparse.Namespace) -> None:
    track_id = play.parse_track_id(args.track)
    session = play.require_session()
    track = session.track(track_id)
    ssid = str(uuid.uuid4())
    info = playback_info(session, track_id, ssid)
    print(f"{track.artist.name} – {track.full_name}")
    print(
        f"Received: {info.get('audioQuality')} {info.get('audioMode')} "
        f"{info.get('assetPresentation')} {info.get('bitDepth')}/{info.get('sampleRate')}"
    )
    url, server = stream_url(info)

    mpv = [
        play.find_mpv(),
        "--no-config",
        "--no-video",
        "--ytdl=no",
        "--cache=yes",
        "--cache-on-disk=no",  # principle 1: nothing on disk
        "--idle=no",  # quit at the end of the track (the bundled mpv may idle)
        "--msg-level=all=error",
        f"--input-ipc-server={PIPE}",
        f"--force-media-title={track.artist.name} – {track.full_name}",
        url,
    ]
    watcher = MpvWatcher()
    print("Playing in mpv (q to stop early)…")
    proc = subprocess.Popen(mpv)
    thread = threading.Thread(target=watcher.run, args=(proc,), daemon=True)
    thread.start()
    proc.wait()
    thread.join(timeout=2)
    if server:
        server.shutdown()

    if watcher.start_ts is None or watcher.last_ts is None:
        sys.exit("Playback never started: no event written.")
    end_pos = watcher.last_pos
    end_ts = watcher.last_ts
    # Polling stops up to 250 ms before the real end: a played-out track ends at its
    # duration, as in the SDK's gapless test.
    if watcher.duration and watcher.duration - end_pos < 1.5:
        end_ts += int((watcher.duration - end_pos) * 1000)
        end_pos = watcher.duration

    event = {
        "group": "play_log",
        "name": "playback_session",
        "version": 2,
        "payload": {
            "actions": watcher.actions,
            "actualAssetPresentation": info.get("assetPresentation"),
            "actualAudioMode": info.get("audioMode"),
            "actualProductId": str(info.get("trackId", track_id)),
            "actualQuality": info.get("audioQuality"),
            "endAssetPosition": round(end_pos, 3),
            "endTimestamp": end_ts,
            "isPostPaywall": info.get("assetPresentation") == "FULL",
            "playbackSessionId": ssid,
            "productType": "TRACK",
            "requestedProductId": str(track_id),
            "sourceId": str(track.album.id) if track.album else "",
            "sourceType": "ALBUM",
            "startAssetPosition": 0,
            "startTimestamp": watcher.start_ts,
        },
    }
    Path(args.out).write_text(json.dumps(event, indent=2), encoding="utf-8")
    print(f"\nListened {round(end_pos)} s out of {round(watcher.duration or 0)} s. Event written to {args.out}:")
    print(json.dumps(event, indent=2))


# --- send ----------------------------------------------------------------------------


def cmd_send(args: argparse.Namespace) -> None:
    event = json.loads(Path(args.event).read_text(encoding="utf-8"))
    event_id = str(uuid.uuid4())
    sent_ts = now_ms()
    body = {**event, "ts": sent_ts, "uuid": event_id}
    headers_attr = {
        "app-name": "Seaglass",
        "app-version": VERSION_FILE.read_text(encoding="utf-8").strip(),
        "client-id": None,  # filled below
        "consent-category": "NECESSARY",
        "os-name": "Windows",
        "requested-sent-timestamp": str(sent_ts),
        "authorization": "<access token, added at send time>",
    }

    session = play.require_session()
    headers_attr["client-id"] = session.config.client_id
    shown = {**headers_attr, "client-id": "<app client id>"}
    print(f"POST {EVENT_BATCH_URL}")
    print("MessageBody:", json.dumps(body, indent=2))
    print("Headers attribute:", json.dumps(shown, indent=2))
    if args.dry_run:
        print("\nDry run: nothing sent.")
        return

    headers_attr["authorization"] = session.access_token
    entry = "SendMessageBatchRequestEntry.1"
    form = {
        f"{entry}.Id": event_id,
        f"{entry}.MessageBody": json.dumps(body),
        f"{entry}.MessageAttribute.1.Name": "Name",
        f"{entry}.MessageAttribute.1.Value.StringValue": event["name"],
        f"{entry}.MessageAttribute.1.Value.DataType": "String",
        f"{entry}.MessageAttribute.2.Name": "Headers",
        f"{entry}.MessageAttribute.2.Value.DataType": "String",
        f"{entry}.MessageAttribute.2.Value.StringValue": json.dumps(headers_attr),
    }
    resp = requests.post(
        EVENT_BATCH_URL,
        data=urlencode(form),
        headers={
            "Authorization": f"Bearer {session.access_token}",
            "Content-Type": "application/x-www-form-urlencoded",
        },
        timeout=15,
    )
    text = resp.text.replace(session.access_token, "<token>")
    print(f"\nHTTP {resp.status_code}")
    print(text[:2000])
    accepted = re.search(r"<SendMessageBatchResultEntry>.*?<Id>(.*?)</Id>", text, re.S)
    if accepted and accepted.group(1) == event_id:
        print("\nAccepted by the event collector (transport only, not proof the play counts).")
    elif "BatchResultErrorEntry" in text:
        print("\nRejected entry (see SenderFault / Code above).")


def main() -> None:
    play.load_env_file()
    parser = argparse.ArgumentParser(description="Spike #34: TIDAL play log")
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("listen", help="play a track for real and write its playback_session event")
    p.add_argument("track")
    p.add_argument("out")
    p.set_defaults(func=cmd_listen)

    p = sub.add_parser("send", help="post an event written by listen")
    p.add_argument("event")
    p.add_argument("--dry-run", action="store_true")
    p.set_defaults(func=cmd_send)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
