#!/usr/bin/env bats
#
# Unit tests for ani-cli's `process_tobeparsed` (lines 259-277), the
# response-decryption half of the encrypted allanime transport that
# replaced decode_tobeparsed/process_response in 4.15.
#
# Contract:
#   - $1 = raw API response text.
#   - Input without a "tobeparsed" key passes through unchanged.
#   - Otherwise the blob (base64 over [1-byte prefix][12-byte IV]
#     [AES-256-GCM ciphertext||16-byte tag]) is decrypted with
#     $botan_exe / $allanime_key and the plaintext is printed.
#   - A failed decrypt (wrong key / bad tag) prints nothing; stderr is
#     discarded inside the function.
#
# Crypto runs through fake_botan.sh so no system Botan is required.

load '../helpers/loader'

setup() {
    FAKE_BOTAN="$REPO_ROOT/tests/bash/helpers/fake_botan.sh"
    TEST_KEY='000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f'
    TEST_IV='aabbccddeeff001122334455'
    PLAINTEXT='{"sourceUrl":"--174c5d4b4c","sourceName":"Default","priority":1}'

    blob_file="$BATS_TEST_TMPDIR/blob.bin"
    {
        printf '\001'
        printf '%s' "$TEST_IV" | xxd -r -p
        printf '%s' "$PLAINTEXT" |
            sh "$FAKE_BOTAN" cipher --cipher=AES-256/GCM --key="$TEST_KEY" --nonce="$TEST_IV" -
    } >"$blob_file"
    RESPONSE="{\"data\":{\"episode\":{\"tobeparsed\":\"$(base64 -w0 <"$blob_file")\"}}}"
}

@test "process_tobeparsed: non-tobeparsed input passes through unchanged" {
    run bash -c '__ANI_CLI_LIB__=1 ANI_CLI_PLAYER=debug . "'"$ANI_CLI_PATH"'" 2>/dev/null
        process_tobeparsed "$1"' _ '{"data":{"shows":{"edges":[]}}}'
    [ "$status" -eq 0 ]
    [ "$output" = '{"data":{"shows":{"edges":[]}}}' ]
}

@test "process_tobeparsed: empty input returns empty (non-tobeparsed path)" {
    run bash -c '__ANI_CLI_LIB__=1 ANI_CLI_PLAYER=debug . "'"$ANI_CLI_PATH"'" 2>/dev/null
        process_tobeparsed ""' _
    [ "$status" -eq 0 ]
    [ -z "$output" ]
}

@test "process_tobeparsed: decrypts a v3 GCM blob back to the plaintext" {
    run bash -c '__ANI_CLI_LIB__=1 ANI_CLI_PLAYER=debug . "'"$ANI_CLI_PATH"'" 2>/dev/null
        botan_exe="$1"
        botan_version=3
        allanime_key="$2"
        process_tobeparsed "$3"' _ "$FAKE_BOTAN" "$TEST_KEY" "$RESPONSE"
    [ "$status" -eq 0 ]
    [ "$output" = "$PLAINTEXT" ]
}

@test "process_tobeparsed: wrong key yields empty output (GCM auth failure)" {
    wrong_key='ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'
    run bash -c '__ANI_CLI_LIB__=1 ANI_CLI_PLAYER=debug . "'"$ANI_CLI_PATH"'" 2>/dev/null
        botan_exe="$1"
        botan_version=3
        allanime_key="$2"
        process_tobeparsed "$3"' _ "$FAKE_BOTAN" "$wrong_key" "$RESPONSE"
    [ "$status" -eq 0 ]
    [ -z "$output" ]
}
