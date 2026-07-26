#!/usr/bin/env bats
#
# Unit tests for ani-cli's `get_aa_req` (lines 278-307), the
# request-signing half of the encrypted allanime transport added in 4.15.
#
# Contract:
#   - Emits base64 (single line, no wrapping) over
#     [0x01][12-byte IV][AES-256-GCM ciphertext||16-byte tag].
#   - The plaintext is {"v":1,"ts":<ts>,"epoch":<epoch>,"qh":"<hash>"}
#     where ts is the current epoch seconds floored to the 300 s grid,
#     in milliseconds.
#   - The IV is the first 12 bytes of SHA-256("<epoch>:<hash>:<ts>").
#
# The whole check runs in one subprocess: source ani-cli in lib mode, call
# get_aa_req, then invert every framing step with fake_botan.sh and fail
# loudly on the first mismatch.

load '../helpers/loader'

setup() {
    FAKE_BOTAN="$REPO_ROOT/tests/bash/helpers/fake_botan.sh"
    TEST_KEY='000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f'
}

@test "get_aa_req: single-line blob decrypts back to the signed payload with a derived IV" {
    run bash -c '
        __ANI_CLI_LIB__=1 ANI_CLI_PLAYER=debug . "$1" 2>/dev/null || true
        set -e
        botan_exe="$2"
        botan_version=3
        allanime_key="$3"
        allanime_epoch=1700000000

        req="$(get_aa_req)"

        # No embedded newlines: GNU base64 without a no-wrap flag would
        # fold at 76 columns and macOS base64 has no -w at all; the
        # portable pipeline must emit one line everywhere.
        [ "$(printf "%s" "$req" | wc -l)" -eq 0 ] || { echo "wrapped output: $req"; exit 1; }

        raw="$(mktemp)"
        printf "%s" "$req" | base64 -d >"$raw"

        [ "$(dd if="$raw" bs=1 count=1 2>/dev/null | od -A n -t x1 | tr -d " \n")" = "01" ] || { echo "bad prefix byte"; exit 1; }
        iv_hex="$(dd if="$raw" bs=1 skip=1 count=12 2>/dev/null | od -A n -t x1 | tr -d " \n")"
        size="$(wc -c <"$raw")"
        payload="$(dd if="$raw" bs=1 skip=13 count=$((size - 13)) 2>/dev/null |
            "$botan_exe" cipher --decrypt --cipher=AES-256/GCM --key="$allanime_key" --nonce="$iv_hex" -)"
        rm -f "$raw"

        ts="$(printf "%s" "$payload" | sed -nE "s|.*\"ts\":([0-9]+).*|\1|p")"
        [ -n "$ts" ] || { echo "no ts in payload: $payload"; exit 1; }
        [ $((ts % 300000)) -eq 0 ] || { echo "ts not on the 300 s grid: $ts"; exit 1; }

        expected="{\"v\":1,\"ts\":$ts,\"epoch\":1700000000,\"qh\":\"$allanime_query_hash\"}"
        [ "$payload" = "$expected" ] || { echo "payload mismatch: $payload != $expected"; exit 1; }

        want_iv="$(printf "%s" "1700000000:$allanime_query_hash:$ts" |
            "$botan_exe" hash --no-fsname | cut -c1-24 | tr "[:upper:]" "[:lower:]")"
        [ "$iv_hex" = "$want_iv" ] || { echo "iv mismatch: $iv_hex != $want_iv"; exit 1; }
    ' _ "$ANI_CLI_PATH" "$FAKE_BOTAN" "$TEST_KEY"
    [ "$status" -eq 0 ]
    [ -z "$output" ]
}
