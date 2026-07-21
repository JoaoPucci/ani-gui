#!/usr/bin/env bats
#
# Tests for ani-cli's `search_anime` (lines 313-319) with mocked curl.
#
# Contract:
#   - $1 = url-encoded search query (spaces as +).
#   - Calls curl POST to ${allanime_api}/api with a GraphQL "search" query.
#   - Parses the response shows.edges[*] into one line per result:
#         <id>\t<name> (<count> episodes) (<year>)
#     The year comes from airedStart.year (ani-cli 4.14.5). Edges
#     without one keep the pre-4.14.5 shape (no year suffix) — a fork
#     patch: upstream drops them, but the GUI passes a 1-based -S
#     index computed against its own full scraper list, so a dropped
#     row would shift every later index onto the wrong anime.
#   - $count is taken from availableEpisodes.${mode} where mode is sub|dub.
#
# We mock curl to return canned JSON fixtures so tests are hermetic.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    # shellcheck source=/dev/null
    load '../helpers/curl_mock'
    export CURL_MOCK_LOG="$BATS_TEST_TMPDIR/curl.log"
    : >"$CURL_MOCK_LOG"
    mode='sub'
}

@test "search_anime: parses four sub results from canned response" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/search_one_piece.json"
    output=$(search_anime "one+piece")
    line_count=$(printf '%s\n' "$output" | wc -l | tr -d ' ')
    [ "$line_count" -eq 4 ]
    # First edge in the fixture: the main TV series with 1100 sub episodes.
    [[ "$output" == *"ReooPAxPMsHM4KPMY"$'\t'"One Piece (1100 episodes) (1999)"* ]]
    # Second edge: a film, single episode.
    [[ "$output" == *"yWebgvMsxR8FAEpw9"$'\t'"One Piece Movie 14: Stampede (1 episodes) (2019)"* ]]
}

@test "search_anime: keeps a result whose airedStart is null (no year suffix)" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/search_one_piece.json"
    output=$(search_anime "one+piece")
    # Fourth edge: a stub row with airedStart null, as allmanga returns
    # for older/uncatalogued entries. It must stay in the list (in the
    # pre-4.14.5 shape) or the GUI's -S index misaligns.
    [[ "$output" == *"nQstubNoYearRow01"$'\t'"One Piece: Recap Special (1 episodes)"$'\n'* || "$output" == *"nQstubNoYearRow01"$'\t'"One Piece: Recap Special (1 episodes)" ]]
}

@test "search_anime: dub mode picks the dub episode count" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/search_one_piece.json"
    mode='dub'
    output=$(search_anime "one+piece")
    # First edge had dub=1085 in the fixture.
    [[ "$output" == *"One Piece (1085 episodes) (1999)"* ]]
}

@test "search_anime: empty result set produces no lines" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/search_empty.json"
    output=$(search_anime "no+such+anime")
    [ -z "$output" ]
}

@test "search_anime: passes the query in the POST body to the allanime api" {
    export CURL_MOCK_RESPONSE="$FIXTURES_DIR/allanime/search_one_piece.json"
    search_anime "naruto+shippuden" >/dev/null
    # Inspect what curl was called with.
    log=$(cat "$CURL_MOCK_LOG")
    # The script POSTs to ${allanime_api}/api with the query embedded in --data.
    [[ "$log" == *"-X POST"* ]]
    [[ "$log" == *"naruto+shippuden"* ]]
    [[ "$log" == *"https://api.allanime.day/api"* ]]
}
