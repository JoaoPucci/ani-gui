#!/bin/sh
#
# Mock `curl` shim placed on PATH for acceptance tests. Pattern-matches the
# combined arguments and returns the appropriate fixture from
# $CURL_FIXTURE_DIR. See tests/bash/acceptance/*.bats for callers.
#
# ani-cli 5.0 resolves its curl through dep_ch_failover, preferring
# curl-impersonate binaries (curl_firefox135 first) over plain curl, so
# callers install this file under BOTH names — otherwise a developer
# machine with real curl-impersonate would route test traffic to the
# live site.
#
# Routing rules (first match wins), mirroring the anidb.app flow:
#   - browse query containing "nohit" → browse_empty.html (a results
#     page with no anime anchors, for the "No results found!" path).
#     Substring match: the Rust driver's `--` separator rides into the
#     query ("--+nohit"), so exact q= matching would miss it.
#   - browse query containing "cloudflare" → browse_cloudflare.html (a
#     "Just a moment" interstitial, for the blocked-by-cloudflare die)
#   - browse?q=…          → browse_one_piece.html (three anchors; the
#     third carries an &#039; entity in its alt title)
#   - /api/frontend/anime/<id>/episodes  → episodes_one_piece.json
#   - /api/frontend/episode/<id>/languages → languages_op.json (jpn +
#     eng embeds, so both --dub and default sub resolve)
#   - embed.example/…     → embed_op.html (jwplayer setup carrying the
#     master-playlist URL in `file: '…'`)
#   - master.m3u8         → master_op.m3u8 (two variants, 1080 + 720)
#   - anidb.app/anime/…   → detail_one_piece.html (MAL link + Seasons
#     block, for anidb_desc). Checked after the /api/frontend routes,
#     which share the host.
#   - raw.githubusercontent.com → update_remote (update_script tests
#     write this fixture themselves)
#   - otherwise → fail loudly to surface unmocked calls
#
# When -o <file> is present the fixture is written there instead of
# stdout, as real curl would.

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
    *browse\?q=*nohit*)
        emit "$fixtures/browse_empty.html"
        ;;
    *browse\?q=*cloudflare*)
        emit "$fixtures/browse_cloudflare.html"
        ;;
    *browse?q=*)
        emit "$fixtures/browse_one_piece.html"
        ;;
    */api/frontend/anime/*/episodes*)
        emit "$fixtures/episodes_one_piece.json"
        ;;
    */api/frontend/episode/*/languages*)
        emit "$fixtures/languages_op.json"
        ;;
    *embed.example*)
        emit "$fixtures/embed_op.html"
        ;;
    *master.m3u8*)
        emit "$fixtures/master_op.m3u8"
        ;;
    *anidb.app/anime/*)
        emit "$fixtures/detail_one_piece.html"
        ;;
    *raw.githubusercontent.com*)
        emit "$fixtures/update_remote"
        ;;
    *)
        printf 'curl shim: no fixture for: %s\n' "$args" >&2
        exit 1
        ;;
esac
