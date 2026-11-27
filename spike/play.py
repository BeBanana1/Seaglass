"""Spike phase 0: TIDAL sign-in, inspecting a track's manifest, playback in mpv.

Goal: check that DRM-free, playable streams are returned for the client ID in use,
and note the formats actually received. Throwaway script.

Usage:
    python spike/play.py login [--pkce]
    python spike/play.py logout
    python spike/play.py search "<query>"
    python spike/play.py probe <track id or URL>
    python spike/play.py play <track id or URL> [--quality ...] [--exclusive]
    python spike/play.py serve <track id or URL> [--quality ...] [--port 8765]   (Android spike, #24)

The refresh token is stored in the Windows Credential Manager (keyring), never
printed or written to disk. No audio stream is written to disk: the DASH manifest is
served from memory through a small local HTTP server.
"""

from __future__ import annotations

import argparse
import base64
import html
import http.server
import json
import os
import re
import shutil
import subprocess
import sys
import threading
import urllib.request
from pathlib import Path

import keyring
import keyring.errors
import tidalapi
from tidalapi.media import Quality

KEYRING_SERVICE = "seaglass-spike"
ENV_FILE = Path(__file__).resolve().parent.parent / ".env"

QUALITIES = {
    "low": Quality.low_96k,
    "high": Quality.low_320k,
    "lossless": Quality.high_lossless,
    "hires": Quality.hi_res_lossless,
}

# DRM markers in an MPD: ContentProtection element, CENC attributes, Widevine system
# UUID. A bare `xmlns:cenc` is present even on clear streams: it is not a marker.
DRM_MARKERS = (
    "<ContentProtection",
    "cenc:default_KID",
    "cenc:pssh",
    "edef8ba9-79d6-4ace-a3c8-27dcd51d21ed",
)
# MP4 boxes revealing encrypted content in the initialization segment.
ENCRYPTED_BOXES = (b"pssh", b"tenc", b"sinf", b"enca")

# Activity of the `spike/android/` test app (#24).
ANDROID_SPIKE_ACTIVITY = "ch.bebanana.seaglass.spike/.MainActivity"


# --- Configuration -------------------------------------------------------------------


def load_env_file() -> None:
    """Load a minimal .env (KEY=value) without overwriting existing variables."""
    if not ENV_FILE.exists():
        return
    for line in ENV_FILE.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip().strip('"'))


def make_session(quality: str = Quality.hi_res_lossless) -> tidalapi.Session:
    config = tidalapi.Config(quality=quality)
    # Overridable client ID; otherwise the one bundled with tidalapi.
    if os.environ.get("TIDAL_CLIENT_ID"):
        config.client_id = os.environ["TIDAL_CLIENT_ID"]
        config.client_secret = os.environ.get("TIDAL_CLIENT_SECRET", config.client_id)
    return tidalapi.Session(config)


# --- Authentification ----------------------------------------------------------------


def save_tokens(session: tidalapi.Session) -> None:
    # Only the refresh token is kept: it fits within the Credential Manager size limit,
    # and the access token is regenerated at every launch.
    keyring.set_password(KEYRING_SERVICE, "refresh_token", session.refresh_token)
    keyring.set_password(KEYRING_SERVICE, "is_pkce", "1" if session.is_pkce else "0")


def restore_session(session: tidalapi.Session) -> bool:
    refresh_token = keyring.get_password(KEYRING_SERVICE, "refresh_token")
    if not refresh_token:
        return False
    session.is_pkce = keyring.get_password(KEYRING_SERVICE, "is_pkce") == "1"
    try:
        if not session.token_refresh(refresh_token):
            return False
    except Exception as exc:  # noqa: BLE001 - spike: we just want a readable message
        print(f"Could not refresh the token: {type(exc).__name__}")
        return False
    return session.load_oauth_session(
        session.token_type,
        session.access_token,
        refresh_token,
        session.expiry_time,
        is_pkce=session.is_pkce,
    )


def require_session(quality: str = Quality.hi_res_lossless) -> tidalapi.Session:
    session = make_session(quality)
    if not restore_session(session):
        sys.exit("No valid session. Run this first: python spike/play.py login")
    return session


def cmd_login(args: argparse.Namespace) -> None:
    session = make_session()
    if args.pkce:
        # PKCE flow ("Android" client): required for hi-res according to tidalapi.
        session.login_pkce()
    else:
        # "Device" flow: link to open in the browser, no password typed here.
        session.login_oauth_simple()
    if not session.check_login():
        sys.exit("Sign-in failed.")
    save_tokens(session)
    print(f"Signed in (country: {session.country_code}, PKCE: {session.is_pkce}).")
    print(f"Subscription: {subscription_label(session)}")


def cmd_logout(_args: argparse.Namespace) -> None:
    for name in ("refresh_token", "is_pkce"):
        try:
            keyring.delete_password(KEYRING_SERVICE, name)
        except keyring.errors.PasswordDeleteError:
            pass
    print("Credentials removed from the Credential Manager.")


def subscription_label(session: tidalapi.Session) -> str:
    try:
        sub = session.request.request(
            "GET", f"users/{session.user.id}/subscription"
        ).json()
        return f"{sub.get('subscription', {}).get('type')} / {sub.get('highestSoundQuality')}"
    except Exception as exc:  # noqa: BLE001
        return f"unknown ({type(exc).__name__})"


# --- Stream inspection ---------------------------------------------------------------


def parse_track_id(value: str) -> int:
    match = re.search(r"(?:track/)?(\d+)", value)
    if not match:
        sys.exit(f"Invalid track ID: {value}")
    return int(match.group(1))


def describe_stream(stream: tidalapi.media.Stream) -> dict:
    """Summarize a stream and detect any DRM directly in the raw manifest.

    tidalapi does not detect encryption in DASH manifests (a TODO in the library),
    hence the search for markers in the XML.
    """
    raw = base64.b64decode(stream.manifest).decode("utf-8", errors="replace")
    info = {
        "quality": stream.audio_quality,
        "mode": stream.audio_mode,
        "manifest": stream.manifest_mime_type,
        "bit_depth": stream.bit_depth,
        "sample_rate": stream.sample_rate,
        "codec": None,
        "encryption": None,
        "raw": raw,
    }
    if stream.is_bts:
        bts = json.loads(raw)
        info["codec"] = bts.get("codecs")
        info["encryption"] = bts.get("encryptionType") or "NONE"
        info["urls"] = bts.get("urls", [])
    elif stream.is_mpd:
        codecs = re.search(r'codecs="([^"]+)"', raw)
        info["codec"] = codecs.group(1) if codecs else "?"
        found = [m for m in DRM_MARKERS if m.lower() in raw.lower()]
        init = re.search(r'initialization="([^"]+)"', raw)
        if init:
            found += init_segment_markers(html.unescape(init.group(1)))
        info["encryption"] = "DRM: " + ", ".join(found) if found else "NONE"
    else:
        info["encryption"] = "unknown manifest format"
    return info


def init_segment_markers(url: str) -> list[str]:
    """Read the initialization segment (a few KB, in memory only) and look in it for the
    CENC encryption MP4 boxes."""
    try:
        with urllib.request.urlopen(url, timeout=15) as resp:
            data = resp.read(256 * 1024)
    except OSError as exc:
        return [f"init unreadable ({type(exc).__name__})"]
    return [f"init:{box.decode()}" for box in ENCRYPTED_BOXES if box in data]


def is_drm(info: dict) -> bool:
    return info["encryption"] not in ("NONE", None)


def cmd_probe(args: argparse.Namespace) -> None:
    track_id = parse_track_id(args.track)
    session = require_session()
    print(f"Subscription: {subscription_label(session)} (PKCE: {session.is_pkce})")
    track = session.track(track_id)
    print(f"{track.artist.name} – {track.full_name}  [advertised: {track.audio_quality}, tags: {track.media_metadata_tags}]\n")

    header = f"{'requested':<9} {'received':<16} {'manifest':<26} {'codec':<10} {'format':<12} encryption"
    print(header)
    print("-" * len(header))
    for label, quality in QUALITIES.items():
        session.audio_quality = quality
        try:
            info = describe_stream(track.get_stream())
        except Exception as exc:  # noqa: BLE001
            print(f"{label:<9} error: {type(exc).__name__}: {exc}")
            continue
        fmt = f"{info['bit_depth']}/{info['sample_rate']}"
        print(
            f"{label:<9} {info['quality']:<16} {info['manifest']:<26} "
            f"{str(info['codec']):<10} {fmt:<12} {info['encryption']}"
        )
        if args.verbose:
            print(redact_urls(info["raw"]), "\n")


def redact_urls(text: str) -> str:
    # Stream URLs are signed: the signature is masked before display.
    return re.sub(r"(https?://[^\s\"'<>?]+)\?[^\s\"'<>]*", r"\1?<signature redacted>", text)


def cmd_search(args: argparse.Namespace) -> None:
    session = require_session()
    results = session.search(args.query, models=[tidalapi.media.Track], limit=15)
    for track in results["tracks"]:
        tags = ",".join(track.media_metadata_tags or [])
        print(f"{track.id:>12}  {tags:<30} {track.artist.name} – {track.full_name}")


# --- Playback ------------------------------------------------------------------------


class _ManifestHandler(http.server.BaseHTTPRequestHandler):
    manifest: bytes = b""

    def do_GET(self) -> None:  # noqa: N802 - name imposed by http.server
        self.send_response(200)
        self.send_header("Content-Type", "application/dash+xml")
        self.send_header("Content-Length", str(len(self.manifest)))
        self.end_headers()
        self.wfile.write(self.manifest)

    def log_message(self, *_args) -> None:
        pass


def serve_manifest(mpd: str, port: int = 0) -> tuple[http.server.ThreadingHTTPServer, str]:
    """Serve the MPD from memory on 127.0.0.1, so that mpv/ffmpeg reads it without a file."""
    handler = type("Handler", (_ManifestHandler,), {"manifest": mpd.encode("utf-8")})
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, f"http://127.0.0.1:{server.server_port}/manifest.mpd"


def find_mpv() -> str:
    path = os.environ.get("SEAGLASS_MPV_PATH") or shutil.which("mpv")
    if not path:
        sys.exit("mpv not found: add it to PATH or set SEAGLASS_MPV_PATH.")
    return path


def cmd_play(args: argparse.Namespace) -> None:
    track_id = parse_track_id(args.track)
    session = require_session(QUALITIES[args.quality])
    track = session.track(track_id)
    info = describe_stream(track.get_stream())
    print(f"{track.artist.name} – {track.full_name}")
    print(f"Received: {info['quality']} {info['codec']} {info['bit_depth']}/{info['sample_rate']} ({info['manifest']})")

    if is_drm(info):
        sys.exit(f"Protected stream ({info['encryption']}): playback refused. End of the spike, let's discuss.")

    server = None
    if info["manifest"].startswith("application/dash+xml"):
        server, url = serve_manifest(info["raw"])
    elif info.get("urls"):
        url = info["urls"][0]
    else:
        sys.exit("No playable URL in the manifest.")

    mpv_args = [
        find_mpv(),
        "--no-config",
        "--no-video",
        "--ytdl=no",
        "--ao=wasapi",
        "--gapless-audio=weak",
        "--cache=yes",
        "--cache-on-disk=no",  # principle 1: nothing on disk
        "--msg-level=all=status,ao=v,ffmpeg/demuxer=warn",
        f"--force-media-title={track.artist.name} – {track.full_name}",
    ]
    if args.exclusive:
        mpv_args += ["--audio-exclusive=yes", "--volume=100", "--replaygain=no"]
    mpv_args.append(url)

    print("Starting mpv (q to quit)…\n")
    try:
        subprocess.run(mpv_args, check=False)
    finally:
        if server:
            server.shutdown()


def cmd_serve(args: argparse.Namespace) -> None:
    """Serve a track's MPD for the Android spike (#24).

    The server stays on the PC's 127.0.0.1; `adb reverse` makes it reachable at the same
    `127.0.0.1:<port>` on the phone, without opening anything on the local network.
    """
    track_id = parse_track_id(args.track)
    session = require_session(QUALITIES[args.quality])
    track = session.track(track_id)
    info = describe_stream(track.get_stream())
    print(f"{track.artist.name} – {track.full_name}")
    print(f"Received: {info['quality']} {info['codec']} {info['bit_depth']}/{info['sample_rate']} ({info['manifest']})")

    if is_drm(info):
        sys.exit(f"Protected stream ({info['encryption']}): playback refused.")
    if not info["manifest"].startswith("application/dash+xml"):
        sys.exit("Non-DASH manifest (BTS): sign in with --pkce.")

    server, url = serve_manifest(info["raw"], args.port)
    print("\nOn the phone (USB debugging enabled):")
    print(f"  adb reverse tcp:{args.port} tcp:{args.port}")
    print(f"  adb shell am start -a android.intent.action.VIEW -n {ANDROID_SPIKE_ACTIVITY} -d {url}")
    print("\nSegment URLs are signed and expire: rerun the command if playback fails.")
    print("Ctrl+C to stop.")
    try:
        threading.Event().wait()
    except KeyboardInterrupt:
        pass
    finally:
        server.shutdown()


# --- Entry point ---------------------------------------------------------------------


def main() -> None:
    load_env_file()
    parser = argparse.ArgumentParser(description="Seaglass spike: TIDAL → mpv")
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("login", help="sign in (device link, or --pkce)")
    p.add_argument(
        "--pkce",
        action="store_true",
        help="PKCE flow: the only one that gives FLAC (the device flow caps at AAC 320)",
    )
    p.set_defaults(func=cmd_login)

    p = sub.add_parser("logout", help="remove the stored credentials")
    p.set_defaults(func=cmd_logout)

    p = sub.add_parser("search", help="search for tracks")
    p.add_argument("query")
    p.set_defaults(func=cmd_search)

    p = sub.add_parser("probe", help="list the formats received for each quality")
    p.add_argument("track", help="track ID or URL")
    p.add_argument("-v", "--verbose", action="store_true", help="print the raw manifest")
    p.set_defaults(func=cmd_probe)

    p = sub.add_parser("play", help="play a track in mpv")
    p.add_argument("track", help="track ID or URL")
    p.add_argument("--quality", choices=QUALITIES, default="hires")
    p.add_argument("--exclusive", action="store_true", help="WASAPI exclusive (bit-perfect)")
    p.set_defaults(func=cmd_play)

    p = sub.add_parser("serve", help="serve a track's MPD to the Android spike (adb reverse)")
    p.add_argument("track", help="track ID or URL")
    p.add_argument("--quality", choices=QUALITIES, default="lossless")
    p.add_argument("--port", type=int, default=8765)
    p.set_defaults(func=cmd_serve)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
