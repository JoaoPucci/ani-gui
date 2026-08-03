#!/usr/bin/env bats
#
# Tests for ani-cli's `anidb_episodes` (5.0's episode-list fetch).
#
# Contract:
#   - Takes the anime slug ("one-piece-69"); the numeric tail after the
#     last hyphen is the id the episodes API is keyed on.
#   - GETs $episodes_api (/api/frontend/anime/<id>/episodes) through
#     $curl_exe; the response is a JSON array of episode objects.
#   - Emits "episode_db_id \t episode_number" per object, in order.
#     Callers keep the pair map in $episode_maps (anidb_m3u8 resolves
#     the played number back to the db id) and cut -f2 for $ep_list.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    export CURL_FIXTURE_DIR="$FIXTURES_DIR/anidb"
    curl_exe="$REPO_ROOT/tests/bash/helpers/curl_shim.sh"
}

@test "anidb_episodes: emits id/number pairs for every episode" {
    output=$(anidb_episodes "one-piece-69")
    expected="9001"$'\t'"1"$'\n'"9002"$'\t'"2"$'\n'"9003"$'\t'"3"
    [ "$output" = "$expected" ]
}

@test "anidb_episodes: requests the numeric tail of the slug, not the slug" {
    # The fetch runs inside a command substitution, so the probe
    # records through a file rather than a variable.
    probe() {
        printf '%s' "$*" >"$BATS_TEST_TMPDIR/url"
        printf '[]'
    }
    curl_exe=probe
    anidb_episodes "one-piece-69" || true
    url=$(cat "$BATS_TEST_TMPDIR/url")
    [[ "$url" == *"/api/frontend/anime/69/episodes"* ]]
    [[ "$url" != *"one-piece"* ]]
}

@test "anidb_episodes: an empty response yields no output" {
    probe() { printf '[]'; }
    curl_exe=probe
    output=$(anidb_episodes "one-piece-69")
    [ -z "$output" ]
}
