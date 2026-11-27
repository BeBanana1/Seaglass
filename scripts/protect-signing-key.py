"""Updater signing key (Tauri format): re-encryption and password check (#87).

Re-encrypting keeps the same key (same identifier, same public key): installed versions
keep accepting updates. Only the encryption of the file changes.

Format: a Tauri file is the base64 of a minisign secret key file
("untrusted comment: ..." then a base64 line). That line decodes to 158 bytes:
  sig_alg "Ed" | kdf_alg "Sc" (or "\\0\\0") | cksum_alg "B2" | salt (32) |
  opslimit (u64 LE) | memlimit (u64 LE) | keynum_sk (104) =
  identifier (8) + Ed25519 secret key (64) + BLAKE2b-256 checksum (32)
With "Sc", keynum_sk is XORed with scrypt(password, salt, 104 bytes), with parameters
derived from opslimit / memlimit like libsodium (`pickparams`). `tauri signer generate`
always encrypts: a "passwordless" key is encrypted with an empty password.

Usage (standard library only):
  python -I scripts/protect-signing-key.py <current key> <new key>
      Passwords read from the keyboard, without echo. The current key is never modified;
      the new one is written to a file that must not exist.
  python -I scripts/protect-signing-key.py --check <key>
      Password read from standard input (UTF-8, one line, empty allowed): exit code 0 if it
      opens the key, 1 if not, 2 if the key is unreadable. Used by `release.ps1`.
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
    """Unreadable key, or one in an unexpected format."""


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
            raise KeyError_(f"unreadable key ({e.__class__.__name__}): is it really a Tauri key?")
        if len(data) != KEY_LEN or data[0:2] != b"Ed" or data[4:6] != b"B2":
            raise KeyError_("unexpected key format (minisign Ed25519 / BLAKE2b expected)")
        self.kdf = data[2:4]
        if self.kdf not in (b"Sc", b"\0\0"):
            raise KeyError_(f"unknown derivation algorithm: {self.kdf!r}")
        self.data = data
        self.salt = data[6:38]
        self.opslimit, self.memlimit = struct.unpack("<QQ", data[38:54])

    def open(self, password: str) -> bytes | None:
        """keynum_sk in clear, or None if the password does not open it."""
        sealed = self.data[54:]
        if self.kdf == b"Sc":
            sealed = xor(sealed, stream(password, self.salt, self.opslimit, self.memlimit))
        keynum, sk, chk = sealed[:8], sealed[8:72], sealed[72:]
        digest = hashlib.blake2b(self.data[0:2] + keynum + sk, digest_size=32).digest()
        return sealed if secrets.compare_digest(digest, chk) else None

    def sealed_with(self, keynum_sk: bytes, password: str) -> str:
        """Tauri file of the same key, encrypted with `password` and a new salt."""
        salt = secrets.token_bytes(32)
        limits = struct.pack("<QQ", self.opslimit, self.memlimit)
        resealed = xor(keynum_sk, stream(password, salt, self.opslimit, self.memlimit))
        data = self.data[0:2] + b"Sc" + self.data[4:6] + salt + limits + resealed
        # Same layout as `tauri signer generate`: base64 of the minisign file.
        lines = self.text.split("\n")
        lines[0] = "untrusted comment: rsign encrypted secret key"
        lines[1] = base64.b64encode(data).decode("ascii")
        return base64.b64encode("\n".join(lines).encode("utf-8")).decode("ascii")


def fail(message: str, code: int = 1) -> None:
    print(f"ERROR: {message}", file=sys.stderr)
    sys.exit(code)


def check(path: str) -> None:
    try:
        key = Key(path)
    except KeyError_ as e:
        fail(str(e), 2)
    # PowerShell 5.1 prefixes a UTF-8 BOM to what it sends to a program: stripped here.
    password = sys.stdin.buffer.read().decode("utf-8-sig")
    password = password[:-2] if password.endswith("\r\n") else password.removesuffix("\n")
    sys.exit(0 if key.open(password) is not None else 1)


def protect(src: str, dst: str) -> None:
    if os.path.exists(dst):
        fail(f"{dst} already exists: choose another file")
    try:
        key = Key(src)
    except KeyError_ as e:
        fail(str(e), 2)
    current = getpass.getpass("Current password (Enter if there is none): ")
    keynum_sk = key.open(current)
    if keynum_sk is None:
        fail("current password is wrong (invalid checksum)")
    print(f"Key opened, identifier {keynum_sk[:8][::-1].hex().upper()}.")

    new = getpass.getpass(f"New password (at least {MIN_PASSWORD} characters): ")
    if len(new) < MIN_PASSWORD:
        fail(f"password too short (fewer than {MIN_PASSWORD} characters)")
    if getpass.getpass("Confirm the new password: ") != new:
        fail("the two passwords differ")

    wrapped = key.sealed_with(keynum_sk, new)
    with open(dst, "x", encoding="ascii", newline="") as f:
        f.write(wrapped)
    # Read back: the new key must open with the new password, and be the same key.
    if Key(dst).open(new) != keynum_sk:
        os.remove(dst)
        fail("reading back the new key failed: nothing was written")
    print(f"Re-encrypted key written to {dst}.")


def main() -> None:
    args = sys.argv[1:]
    if len(args) == 2 and args[0] == "--check":
        check(args[1])
    elif len(args) == 2 and not args[0].startswith("-"):
        protect(args[0], args[1])
    else:
        fail("usage: protect-signing-key.py <current key> <new key>\n"
             "        protect-signing-key.py --check <key>  (password on standard input)")


if __name__ == "__main__":
    main()
