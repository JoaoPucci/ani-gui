#!/bin/sh
#
# botan(1) stand-in for the bats suites. ani-cli 4.15 shells out to a real
# Botan CLI for the allanime API crypto; the tests must not depend on a
# system Botan install, so the harness puts this shim on PATH as `botan`
# (acceptance) or in $botan_exe (unit). It implements exactly the four
# invocations ani-cli makes, with real AES-256-GCM / SHA-256 via
# python3-cryptography:
#
#   fake_botan.sh --version
#       -> "3.0.0-fake". ani-cli reads only the first character to pick
#          the Botan-3 argument syntax, so only that syntax exists here.
#   fake_botan.sh hash --no-fsname
#       -> SHA-256 of stdin as uppercase hex + newline (botan's format).
#   fake_botan.sh hex_dec -
#       -> hex text on stdin decoded to raw bytes on stdout.
#   fake_botan.sh cipher --cipher=AES-256/GCM [--decrypt] --key=<hex> \
#       --nonce=<hex> -
#       -> encrypt: stdout is ciphertext||tag(16). decrypt: stdin is
#          ciphertext||tag(16), stdout is the plaintext. A bad tag exits
#          nonzero with empty stdout, matching real botan's failure shape.

set -eu

if [ "${1:-}" = "--version" ]; then
    printf '3.0.0-fake\n'
    exit 0
fi

exec python3 -c '
import sys
import hashlib

args = sys.argv[1:]
cmd = args[0] if args else ""

if cmd == "hash":
    data = sys.stdin.buffer.read()
    sys.stdout.write(hashlib.sha256(data).hexdigest().upper() + "\n")
elif cmd == "hex_dec":
    text = "".join(sys.stdin.read().split())
    sys.stdout.buffer.write(bytes.fromhex(text))
elif cmd == "cipher":
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

    key = nonce = None
    decrypt = False
    for arg in args[1:]:
        if arg == "--decrypt":
            decrypt = True
        elif arg.startswith("--key="):
            key = bytes.fromhex(arg[len("--key="):])
        elif arg.startswith("--nonce="):
            nonce = bytes.fromhex(arg[len("--nonce="):])
    if key is None or nonce is None:
        sys.stderr.write("fake_botan: cipher needs --key and --nonce\n")
        sys.exit(2)
    data = sys.stdin.buffer.read()
    gcm = AESGCM(key)
    out = gcm.decrypt(nonce, data, None) if decrypt else gcm.encrypt(nonce, data, None)
    sys.stdout.buffer.write(out)
else:
    sys.stderr.write("fake_botan: unsupported invocation: " + repr(args) + "\n")
    sys.exit(2)
' "$@"
