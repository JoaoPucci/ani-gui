#!/bin/sh
#
# Mock `curl` shim placed on PATH for acceptance tests. Pattern-matches the
# combined arguments and returns the appropriate fixture from
# $CURL_FIXTURE_DIR. See tests/bash/acceptance/*.bats for callers.
#
# Routing rules (first match wins):
#   - body contains "episodeString" → episode_blob.json (a synthesized
#     allanime API response with a valid encrypted tobeparsed blob)
#   - GET URL contains both "variables=" and "extensions=" → same as above
#   - body contains "showId" → episodes_short.json
#   - body contains '"search"' → search_one_piece.json
#   - URL under the CDN's entry/app. path → keys_app.js (fetch_keys'
#     app bundle, which names the chunk file)
#   - URL under the CDN's chunks/ path → keys_chunk.js (carries the
#     64-hex key mask fetch_keys XORs against the page's partB)
#   - GET URL hits "allanime.day/" but not "/api" → embed_simple.json
#     (the wixmp default embed). Checked before the referrer-page rule:
#     embed fetches pass `-e` with the referrer host, which would
#     otherwise swallow them.
#   - URL hits the referrer host (mkissa.to) → keys_page.html (serves
#     epoch + partB + the app bundle URL to fetch_keys)
#   - otherwise → fail loudly to surface unmocked calls
#
# fetch_keys fetches the referrer page with `-o <file>`; when -o is
# present the fixture is written there instead of stdout, as real curl
# would.

set -eu

args="$*"
fixtures="${CURL_FIXTURE_DIR:?CURL_FIXTURE_DIR not set}"

out=''
prev=''
for arg in "$@"; do
    [ "$prev" = "-o" ] && out="$arg"
    prev="$arg"
done

emit() {
    if [ -n "$out" ]; then
        cat "$1" >"$out"
    else
        cat "$1"
    fi
}

case "$args" in
    *episodeString*)
        emit "$fixtures/episode_blob.json"
        ;;
    *variables=*extensions=*)
        emit "$fixtures/episode_blob.json"
        ;;
    *showId*)
        emit "$fixtures/episodes_short.json"
        ;;
    *'"search"'*)
        emit "$fixtures/search_one_piece.json"
        ;;
    */entry/app.*)
        emit "$fixtures/keys_app.js"
        ;;
    */chunks/*)
        emit "$fixtures/keys_chunk.js"
        ;;
    *allanime.day/*)
        # Embed page fetch (any provider path) → return the same simple
        # wixmp embed for every fetch. Provider branches that cannot decode
        # this response just emit nothing, which is the point — only one
        # provider needs to return a usable link for select_quality to pick.
        emit "$fixtures/embed_simple.json"
        ;;
    *mkissa.to*)
        emit "$fixtures/keys_page.html"
        ;;
    *)
        printf 'curl shim: no fixture for: %s\n' "$args" >&2
        exit 1
        ;;
esac
