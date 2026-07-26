#!/usr/bin/env bats
#
# Property test: process_tobeparsed(build_blob(pt, key, iv)) == pt for
# random plaintexts, keys, and IVs.
#
# bats has no native shrinking; we emulate property tests with a small
# generator harness. Blobs are built with fake_botan.sh — the same
# AES-256-GCM the function decrypts with — so this pins the framing
# (prefix/IV/ciphertext/tag offsets and the base64 envelope), not just
# one hand-rolled fixture. Plaintexts are base64 text so they stay
# JSON- and shell-safe; length varies 1..200 chars.
#
# The whole loop runs in one subprocess so a mismatch (or a removed
# function) fails the test through the exit status.

load '../helpers/loader'

setup() {
    FAKE_BOTAN="$REPO_ROOT/tests/bash/helpers/fake_botan.sh"
}

@test "process_tobeparsed: GCM round-trip for 25 random payload/key/iv triples" {
    run bash -c '
        __ANI_CLI_LIB__=1 ANI_CLI_PLAYER=debug . "$1" 2>/dev/null || true
        set -e
        botan_exe="$2"
        botan_version=3

        iter=0
        while [ "$iter" -lt 25 ]; do
            len=$(((RANDOM % 200) + 1))
            pt="$(head -c 256 /dev/urandom | base64 -w0 | cut -c1-"$len")"
            allanime_key="$(head -c 32 /dev/urandom | od -A n -t x1 | tr -d " \n")"
            iv_hex="$(head -c 12 /dev/urandom | od -A n -t x1 | tr -d " \n")"

            blob="$(mktemp)"
            {
                printf "\001"
                printf "%s" "$iv_hex" | xxd -r -p
                printf "%s" "$pt" | "$botan_exe" cipher --cipher=AES-256/GCM --key="$allanime_key" --nonce="$iv_hex" -
            } >"$blob"
            resp="{\"data\":{\"tobeparsed\":\"$(base64 -w0 <"$blob")\"}}"
            rm -f "$blob"

            out="$(process_tobeparsed "$resp")"
            if [ "$out" != "$pt" ]; then
                printf "mismatch at iter %s (len=%s):\n  pt:  %s\n  out: %s\n" "$iter" "$len" "$pt" "$out"
                exit 1
            fi
            iter=$((iter + 1))
        done
    ' _ "$ANI_CLI_PATH" "$FAKE_BOTAN"
    [ "$status" -eq 0 ]
    [ -z "$output" ]
}
