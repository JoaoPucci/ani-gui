#!/usr/bin/env bash
#
# Build a synthetic episode_blob.json fixture for the acceptance tests. The
# response shape mirrors what allanime's API returns since the encrypted
# transport arrived in ani-cli 4.15: the GraphQL endpoint serves a
# "tobeparsed" blob — base64 over
#
#   [1-byte prefix][12-byte IV][AES-256-GCM ciphertext || 16-byte tag]
#
# which process_tobeparsed decrypts (via botan; the tests use
# fake_botan.sh) with the key fetch_keys derived.
#
# Usage:
#   blob_builder.sh <output_path>
#
# Hardcoded plaintext:
#   {"sourceUrl":"--174c5d4b4c","sourceName":"Default","priority":1}
#
# 174c5d4b4c decodes (via the substitution table inside provider_init) to
# the path "/test", which the embed-page fetch step reaches at
# https://allanime.day/test. The curl shim returns embed_simple.json for
# any allanime.day/* GET, so the wixmp-default branch produces a usable
# link and the other providers fail silently.
#
# The key mirrors the fetch_keys fixtures the acceptance shim serves:
# keys_chunk.js's 64-hex mask and keys_page.html's partB are the same 32
# bytes (0x00..0x1f), so mask XOR partB derives the all-zero key below.
# partB deliberately isn't all-identical bytes: ani-cli reads it back
# through `od`, which collapses repeated 16-byte lines into a literal
# `*` and would corrupt the derivation.

set -eu

out="${1:?usage: blob_builder.sh <output_path>}"

helpers_dir="$(cd "$(dirname "$0")" && pwd)"
fake_botan="$helpers_dir/fake_botan.sh"

allanime_key='0000000000000000000000000000000000000000000000000000000000000000'
iv_hex='000102030405060708090a0b'
plaintext='{"sourceUrl":"--174c5d4b4c","sourceName":"Default","priority":1}'

tmp=$(mktemp)
{
    # 1-byte prefix (process_tobeparsed skips it unconditionally)
    printf '\001'
    # 12 bytes of IV
    printf '%s' "$iv_hex" | xxd -r -p
    # ciphertext || 16-byte GCM tag
    printf '%s' "$plaintext" |
        sh "$fake_botan" cipher --cipher=AES-256/GCM --key="$allanime_key" --nonce="$iv_hex" -
} >"$tmp"

blob_b64=$(base64 -w0 <"$tmp")
rm -f "$tmp"

cat >"$out" <<EOF
{"data":{"episode":{"episodeString":"1","sourceUrls":[],"tobeparsed":"${blob_b64}","__typename":"Episode"}}}
EOF
