#!/usr/bin/env bats
#
# Tests for ani-cli's `anidb_search` (5.0's provider search).
#
# Contract:
#   - GETs $search_api (anidb.app/browse?q=<query>) through $curl_exe.
#   - Splits the page on `<a href` anchors and extracts
#     "slug-with-id \t title" per anchor via the anime/([a-z0-9-]+-[0-9]+)
#     href capture and the img alt attribute.
#   - Decodes the &#039; and &quot; HTML entities in titles.
#   - A "Just a moment" interstitial (cloudflare) dies instead of
#     returning an empty list; the message suggests curl-impersonate
#     only when plain curl made the request.
#
# Network is hermetic: $curl_exe is pointed at the curl shim, which
# serves tests/fixtures/anidb/ by URL pattern.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    export CURL_FIXTURE_DIR="$FIXTURES_DIR/anidb"
    curl_exe="$REPO_ROOT/tests/bash/helpers/curl_shim.sh"
}

@test "anidb_search: extracts slug and title pairs from the browse page" {
    output=$(anidb_search "one+piece")
    expected="one-piece-69"$'\t'"One Piece"
    [ "$(printf '%s\n' "$output" | head -n 1)" = "$expected" ]
    line_count=$(printf '%s\n' "$output" | wc -l | tr -d ' ')
    [ "$line_count" -eq 3 ]
}

@test "anidb_search: decodes HTML entities in titles" {
    output=$(anidb_search "one+piece")
    # The third fixture anchor carries alt="Gintama&#039;: The Movie".
    printf '%s\n' "$output" | grep -F "gintama-the-movie-4425"$'\t'"Gintama': The Movie" >/dev/null
}

@test "anidb_search: a results page with no anime anchors yields no output" {
    output=$(anidb_search "nohit")
    [ -z "$output" ]
}

@test "anidb_search: a cloudflare interstitial dies instead of returning empty" {
    run bash -c '
        __ANI_CLI_LIB__=1 . "$ANI_CLI_PATH" 2>/dev/null
        export CURL_FIXTURE_DIR="$FIXTURES_DIR/anidb"
        curl_exe="$REPO_ROOT/tests/bash/helpers/curl_shim.sh"
        anidb_search "cloudflare"
    '
    [ "$status" -eq 1 ]
    [[ "$output" == *"Blocked by cloudflare"* ]]
}

@test "anidb_search: the query lands in the browse URL via search_api" {
    # The fetch runs inside a command substitution, so the probe
    # records through a file rather than a variable.
    probe() {
        printf '%s' "$*" >"$BATS_TEST_TMPDIR/url"
        cat "$FIXTURES_DIR/anidb/browse_empty.html"
    }
    curl_exe=probe
    anidb_search "spy+family" || true
    grep -F "anidb.app/browse?q=spy+family" "$BATS_TEST_TMPDIR/url" >/dev/null
}
