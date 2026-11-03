"""Clé de signature de l'updater (format Tauri) : rechiffrement et contrôle du mot de passe (#87).

Rechiffrer garde la même clé (même identifiant, même clé publique) : les versions installées
continuent d'accepter les mises à jour. Seul le chiffrement du fichier change.

Format : un fichier Tauri est le base64 d'un fichier de clé secrète minisign
(« untrusted comment: … » puis une ligne base64). Cette ligne décode en 158 octets :
  sig_alg "Ed" | kdf_alg "Sc" (ou "\\0\\0") | cksum_alg "B2" | sel (32) |
  opslimit (u64 LE) | memlimit (u64 LE) | keynum_sk (104) =
  identifiant (8) + clé secrète Ed25519 (64) + somme BLAKE2b-256 (32)
Avec "Sc", keynum_sk est XORé avec scrypt(mot de passe, sel, 104 octets), paramètres tirés
d'opslimit / memlimit comme libsodium (`pickparams`). `tauri signer generate` chiffre
toujours : une clé « sans mot de passe » l'est avec un mot de passe vide.

Usage (bibliothèque standard seulement) :
  python -I scripts/protect-signing-key.py <clé actuelle> <nouvelle clé>
      Mots de passe lus au clavier, sans écho. La clé actuelle n'est jamais modifiée ;
      la nouvelle est écrite dans un fichier qui ne doit pas exister.
  python -I scripts/protect-signing-key.py --check <clé>
      Mot de passe lu sur l'entrée standard (UTF-8, une ligne, vide permis) : code 0 s'il
      ouvre la clé, 1 sinon, 2 si la clé est illisible. Utilisé par `release.ps1`.
"""

import base64
import getpass
import hashlib
import os
import secrets
import struct
import sys

KEYNUM_LEN = 8 + 64 + 32
KEY_LEN = 6 + 32 + 16 + KEYNUM_LEN
MIN_PASSWORD = 12


class KeyError_(Exception):
    """Clé illisible ou d'un format inattendu."""


def pick_params(opslimit: int, memlimit: int) -> tuple[int, int, int]:
    """`pickparams` de libsodium (crypto_pwhash_scryptsalsa208sha256) : (N, r, p)."""
    opslimit = max(opslimit, 32768)
    r = 8
    if opslimit < memlimit // 32:
        p = 1
        max_n = opslimit // (r * 4)
    else:
        max_n = memlimit // (r * 128)
    n_log2 = 1
    while n_log2 < 63 and (1 << n_log2) <= max_n // 2:
        n_log2 += 1
    if opslimit >= memlimit // 32:
        max_rp = min((opslimit // 4) // (1 << n_log2), 0x3FFFFFFF)
        p = max_rp // r
    return 1 << n_log2, r, p


def stream(password: str, salt: bytes, opslimit: int, memlimit: int) -> bytes:
    n, r, p = pick_params(opslimit, memlimit)
    return hashlib.scrypt(
        password.encode("utf-8"), salt=salt, n=n, r=r, p=p,
        maxmem=128 * n * r * (p + 2), dklen=KEYNUM_LEN,
    )


def xor(a: bytes, b: bytes) -> bytes:
    return bytes(x ^ y for x, y in zip(a, b))


class Key:
    def __init__(self, path: str):
        try:
            raw = open(path, "rb").read().strip()
            self.text = base64.b64decode(raw, validate=True).decode("utf-8")
            data = base64.b64decode(self.text.split("\n")[1].strip(), validate=True)
        except (OSError, ValueError, UnicodeDecodeError, IndexError) as e:
            raise KeyError_(f"clé illisible ({e.__class__.__name__}) : est-ce bien une clé Tauri ?")
        if len(data) != KEY_LEN or data[0:2] != b"Ed" or data[4:6] != b"B2":
            raise KeyError_("format de clé inattendu (minisign Ed25519 / BLAKE2b attendu)")
        self.kdf = data[2:4]
        if self.kdf not in (b"Sc", b"\0\0"):
            raise KeyError_(f"algorithme de dérivation inconnu : {self.kdf!r}")
        self.data = data
        self.salt = data[6:38]
        self.opslimit, self.memlimit = struct.unpack("<QQ", data[38:54])

    def open(self, password: str) -> bytes | None:
        """keynum_sk en clair, ou None si le mot de passe ne l'ouvre pas."""
        sealed = self.data[54:]
        if self.kdf == b"Sc":
            sealed = xor(sealed, stream(password, self.salt, self.opslimit, self.memlimit))
        keynum, sk, chk = sealed[:8], sealed[8:72], sealed[72:]
        digest = hashlib.blake2b(self.data[0:2] + keynum + sk, digest_size=32).digest()
        return sealed if secrets.compare_digest(digest, chk) else None

    def sealed_with(self, keynum_sk: bytes, password: str) -> str:
        """Fichier Tauri de la même clé, chiffrée avec `password` et un nouveau sel."""
        salt = secrets.token_bytes(32)
        limits = struct.pack("<QQ", self.opslimit, self.memlimit)
        resealed = xor(keynum_sk, stream(password, salt, self.opslimit, self.memlimit))
        data = self.data[0:2] + b"Sc" + self.data[4:6] + salt + limits + resealed
        # Même mise en forme que `tauri signer generate` : base64 du fichier minisign.
        lines = self.text.split("\n")
        lines[0] = "untrusted comment: rsign encrypted secret key"
        lines[1] = base64.b64encode(data).decode("ascii")
        return base64.b64encode("\n".join(lines).encode("utf-8")).decode("ascii")


def fail(message: str, code: int = 1) -> None:
    print(f"ERREUR : {message}", file=sys.stderr)
    sys.exit(code)


def check(path: str) -> None:
    try:
        key = Key(path)
    except KeyError_ as e:
        fail(str(e), 2)
    # PowerShell 5.1 préfixe un BOM UTF-8 à ce qu'il envoie à un programme : retiré.
    password = sys.stdin.buffer.read().decode("utf-8-sig")
    password = password[:-2] if password.endswith("\r\n") else password.removesuffix("\n")
    sys.exit(0 if key.open(password) is not None else 1)


def protect(src: str, dst: str) -> None:
    if os.path.exists(dst):
        fail(f"{dst} existe déjà : choisir un autre fichier")
    try:
        key = Key(src)
    except KeyError_ as e:
        fail(str(e), 2)
    current = getpass.getpass("Mot de passe actuel (Entrée s'il n'y en a pas) : ")
    keynum_sk = key.open(current)
    if keynum_sk is None:
        fail("mot de passe actuel incorrect (somme de contrôle invalide)")
    print(f"Clé ouverte, identifiant {keynum_sk[:8][::-1].hex().upper()}.")

    new = getpass.getpass(f"Nouveau mot de passe ({MIN_PASSWORD} caractères au moins) : ")
    if len(new) < MIN_PASSWORD:
        fail(f"mot de passe trop court (moins de {MIN_PASSWORD} caractères)")
    if getpass.getpass("Confirmer le nouveau mot de passe : ") != new:
        fail("les deux mots de passe diffèrent")

    wrapped = key.sealed_with(keynum_sk, new)
    with open(dst, "x", encoding="ascii", newline="") as f:
        f.write(wrapped)
    # Relecture : la nouvelle clé doit s'ouvrir avec le nouveau mot de passe, sur la même clé.
    if Key(dst).open(new) != keynum_sk:
        os.remove(dst)
        fail("relecture de la nouvelle clé en échec : rien n'a été écrit")
    print(f"Clé rechiffrée écrite dans {dst}.")


def main() -> None:
    args = sys.argv[1:]
    if len(args) == 2 and args[0] == "--check":
        check(args[1])
    elif len(args) == 2 and not args[0].startswith("-"):
        protect(args[0], args[1])
    else:
        fail("usage : protect-signing-key.py <clé actuelle> <nouvelle clé>\n"
             "        protect-signing-key.py --check <clé>  (mot de passe sur l'entrée standard)")


if __name__ == "__main__":
    main()
