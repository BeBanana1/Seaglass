"""Spike phase 0 : connexion TIDAL, inspection du manifeste d'un titre, lecture dans mpv.

Objectif : vérifier qu'on obtient des flux lisibles sans DRM pour l'identifiant client
utilisé, et noter les formats réellement reçus. Script jetable.

Usage :
    python spike/play.py login [--pkce]
    python spike/play.py logout
    python spike/play.py search "<requête>"
    python spike/play.py probe <id ou URL de titre>
    python spike/play.py play <id ou URL de titre> [--quality ...] [--exclusive]

Le refresh token est stocké dans le Gestionnaire d'identifiants Windows (keyring),
jamais affiché ni écrit sur disque. Aucun flux audio n'est écrit sur disque : le
manifeste DASH est servi depuis la mémoire via un mini serveur HTTP local.
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

# Marqueurs de DRM dans un MPD : élément ContentProtection, attributs CENC, UUID système
# Widevine. Le simple `xmlns:cenc` est présent même sur les flux en clair : ce n'est pas
# un marqueur.
DRM_MARKERS = (
    "<ContentProtection",
    "cenc:default_KID",
    "cenc:pssh",
    "edef8ba9-79d6-4ace-a3c8-27dcd51d21ed",
)
# Boîtes MP4 révélant un contenu chiffré dans le segment d'initialisation.
ENCRYPTED_BOXES = (b"pssh", b"tenc", b"sinf", b"enca")


# --- Configuration -------------------------------------------------------------------


def load_env_file() -> None:
    """Charge un .env minimal (CLE=valeur) sans écraser les variables existantes."""
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
    # Identifiant client surchargeable ; sinon celui embarqué par tidalapi.
    if os.environ.get("TIDAL_CLIENT_ID"):
        config.client_id = os.environ["TIDAL_CLIENT_ID"]
        config.client_secret = os.environ.get("TIDAL_CLIENT_SECRET", config.client_id)
    return tidalapi.Session(config)


# --- Authentification ----------------------------------------------------------------


def save_tokens(session: tidalapi.Session) -> None:
    # Seul le refresh token est conservé : il tient dans la limite de taille du
    # Credential Manager, et l'access token est régénéré à chaque lancement.
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
    except Exception as exc:  # noqa: BLE001 - spike : on veut juste un message lisible
        print(f"Rafraîchissement du token impossible : {type(exc).__name__}")
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
        sys.exit("Pas de session valide. Lance d'abord : python spike/play.py login")
    return session


def cmd_login(args: argparse.Namespace) -> None:
    session = make_session()
    if args.pkce:
        # Flux PKCE (client « Android ») : nécessaire pour le hi-res selon tidalapi.
        session.login_pkce()
    else:
        # Flux « appareil » : lien à ouvrir dans le navigateur, sans saisie de mot de passe ici.
        session.login_oauth_simple()
    if not session.check_login():
        sys.exit("Échec de la connexion.")
    save_tokens(session)
    print(f"Connecté (pays : {session.country_code}, PKCE : {session.is_pkce}).")
    print(f"Abonnement : {subscription_label(session)}")


def cmd_logout(_args: argparse.Namespace) -> None:
    for name in ("refresh_token", "is_pkce"):
        try:
            keyring.delete_password(KEYRING_SERVICE, name)
        except keyring.errors.PasswordDeleteError:
            pass
    print("Identifiants supprimés du Gestionnaire d'identifiants.")


def subscription_label(session: tidalapi.Session) -> str:
    try:
        sub = session.request.request(
            "GET", f"users/{session.user.id}/subscription"
        ).json()
        return f"{sub.get('subscription', {}).get('type')} / {sub.get('highestSoundQuality')}"
    except Exception as exc:  # noqa: BLE001
        return f"inconnu ({type(exc).__name__})"


# --- Inspection des flux -------------------------------------------------------------


def parse_track_id(value: str) -> int:
    match = re.search(r"(?:track/)?(\d+)", value)
    if not match:
        sys.exit(f"Identifiant de titre invalide : {value}")
    return int(match.group(1))


def describe_stream(stream: tidalapi.media.Stream) -> dict:
    """Résume un flux et détecte un éventuel DRM directement dans le manifeste brut.

    tidalapi ne détecte pas le chiffrement des manifestes DASH (TODO dans la lib),
    d'où la recherche de marqueurs dans le XML.
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
        info["encryption"] = "format de manifeste inconnu"
    return info


def init_segment_markers(url: str) -> list[str]:
    """Lit le segment d'initialisation (quelques Ko, en mémoire seulement) et y cherche
    les boîtes MP4 de chiffrement CENC."""
    try:
        with urllib.request.urlopen(url, timeout=15) as resp:
            data = resp.read(256 * 1024)
    except OSError as exc:
        return [f"init illisible ({type(exc).__name__})"]
    return [f"init:{box.decode()}" for box in ENCRYPTED_BOXES if box in data]


def is_drm(info: dict) -> bool:
    return info["encryption"] not in ("NONE", None)


def cmd_probe(args: argparse.Namespace) -> None:
    track_id = parse_track_id(args.track)
    session = require_session()
    print(f"Abonnement : {subscription_label(session)} (PKCE : {session.is_pkce})")
    track = session.track(track_id)
    print(f"{track.artist.name} – {track.full_name}  [annoncé : {track.audio_quality}, tags : {track.media_metadata_tags}]\n")

    header = f"{'demandé':<9} {'reçu':<16} {'manifeste':<26} {'codec':<10} {'format':<12} chiffrement"
    print(header)
    print("-" * len(header))
    for label, quality in QUALITIES.items():
        session.audio_quality = quality
        try:
            info = describe_stream(track.get_stream())
        except Exception as exc:  # noqa: BLE001
            print(f"{label:<9} erreur : {type(exc).__name__}: {exc}")
            continue
        fmt = f"{info['bit_depth']}/{info['sample_rate']}"
        print(
            f"{label:<9} {info['quality']:<16} {info['manifest']:<26} "
            f"{str(info['codec']):<10} {fmt:<12} {info['encryption']}"
        )
        if args.verbose:
            print(redact_urls(info["raw"]), "\n")


def redact_urls(text: str) -> str:
    # Les URLs de flux sont signées : on masque la signature avant affichage.
    return re.sub(r"(https?://[^\s\"'<>?]+)\?[^\s\"'<>]*", r"\1?<signature masquée>", text)


def cmd_search(args: argparse.Namespace) -> None:
    session = require_session()
    results = session.search(args.query, models=[tidalapi.media.Track], limit=15)
    for track in results["tracks"]:
        tags = ",".join(track.media_metadata_tags or [])
        print(f"{track.id:>12}  {tags:<30} {track.artist.name} – {track.full_name}")


# --- Lecture -------------------------------------------------------------------------


class _ManifestHandler(http.server.BaseHTTPRequestHandler):
    manifest: bytes = b""

    def do_GET(self) -> None:  # noqa: N802 - nom imposé par http.server
        self.send_response(200)
        self.send_header("Content-Type", "application/dash+xml")
        self.send_header("Content-Length", str(len(self.manifest)))
        self.end_headers()
        self.wfile.write(self.manifest)

    def log_message(self, *_args) -> None:
        pass


def serve_manifest(mpd: str) -> tuple[http.server.ThreadingHTTPServer, str]:
    """Sert le MPD depuis la mémoire sur 127.0.0.1, pour que mpv/ffmpeg le lise sans fichier."""
    handler = type("Handler", (_ManifestHandler,), {"manifest": mpd.encode("utf-8")})
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, f"http://127.0.0.1:{server.server_port}/manifest.mpd"


def find_mpv() -> str:
    path = os.environ.get("SEAGLASS_MPV_PATH") or shutil.which("mpv")
    if not path:
        sys.exit("mpv introuvable : ajoute-le au PATH ou définis SEAGLASS_MPV_PATH.")
    return path


def cmd_play(args: argparse.Namespace) -> None:
    track_id = parse_track_id(args.track)
    session = require_session(QUALITIES[args.quality])
    track = session.track(track_id)
    info = describe_stream(track.get_stream())
    print(f"{track.artist.name} – {track.full_name}")
    print(f"Reçu : {info['quality']} {info['codec']} {info['bit_depth']}/{info['sample_rate']} ({info['manifest']})")

    if is_drm(info):
        sys.exit(f"Flux protégé ({info['encryption']}) : lecture refusée. Fin du spike, on en discute.")

    server = None
    if info["manifest"].startswith("application/dash+xml"):
        server, url = serve_manifest(info["raw"])
    elif info.get("urls"):
        url = info["urls"][0]
    else:
        sys.exit("Aucune URL lisible dans le manifeste.")

    mpv_args = [
        find_mpv(),
        "--no-config",
        "--no-video",
        "--ytdl=no",
        "--ao=wasapi",
        "--gapless-audio=weak",
        "--cache=yes",
        "--cache-on-disk=no",  # principe 1 : rien sur disque
        "--msg-level=all=status,ao=v,ffmpeg/demuxer=warn",
        f"--force-media-title={track.artist.name} – {track.full_name}",
    ]
    if args.exclusive:
        mpv_args += ["--audio-exclusive=yes", "--volume=100", "--replaygain=no"]
    mpv_args.append(url)

    print("Lancement de mpv (q pour quitter)…\n")
    try:
        subprocess.run(mpv_args, check=False)
    finally:
        if server:
            server.shutdown()


# --- Point d'entrée ------------------------------------------------------------------


def main() -> None:
    load_env_file()
    parser = argparse.ArgumentParser(description="Spike Seaglass : TIDAL → mpv")
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("login", help="se connecter (lien appareil, ou --pkce)")
    p.add_argument(
        "--pkce",
        action="store_true",
        help="flux PKCE : seul à donner du FLAC (le flux appareil plafonne à l'AAC 320)",
    )
    p.set_defaults(func=cmd_login)

    p = sub.add_parser("logout", help="supprimer les identifiants stockés")
    p.set_defaults(func=cmd_logout)

    p = sub.add_parser("search", help="chercher des titres")
    p.add_argument("query")
    p.set_defaults(func=cmd_search)

    p = sub.add_parser("probe", help="lister les formats reçus pour chaque qualité")
    p.add_argument("track", help="identifiant ou URL du titre")
    p.add_argument("-v", "--verbose", action="store_true", help="afficher le manifeste brut")
    p.set_defaults(func=cmd_probe)

    p = sub.add_parser("play", help="lire un titre dans mpv")
    p.add_argument("track", help="identifiant ou URL du titre")
    p.add_argument("--quality", choices=QUALITIES, default="hires")
    p.add_argument("--exclusive", action="store_true", help="WASAPI exclusif (bit-perfect)")
    p.set_defaults(func=cmd_play)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
