#!/usr/bin/env bats
#
# Tests for ani-cli's `anidb_desc` (5.0's detail-page metadata fetch).
#
# Contract:
#   - GETs the anime detail page (anidb.app/anime/<slug>).
#   - Extracts $mal_id from the MyAnimeList link (ani-skip is keyed on
#     it).
#   - Extracts $seasons ("slug \t title" rows) from the anchors between
#     the >Seasons< and >Details< markers, and sets $seasons_option so
#     the playback menu grows a change_season entry.
#   - A page without a Seasons block leaves $seasons_option unset.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    export CURL_FIXTURE_DIR="$FIXTURES_DIR/anidb"
    curl_exe="$REPO_ROOT/tests/bash/helpers/curl_shim.sh"
    unset mal_id seasons seasons_option
}

@test "anidb_desc: extracts the MAL id from the detail page" {
    anidb_desc "one-piece-69" || true
    [ "$mal_id" = "21" ]
}

@test "anidb_desc: extracts seasons rows and arms the change_season menu entry" {
    anidb_desc "one-piece-69" || true
    printf '%s\n' "$seasons" | grep -F "one-piece-film-red-9021"$'\t'"One Piece Film: Red" >/dev/null
    [ -n "$seasons_option" ]
}

@test "anidb_desc: a page without a Seasons block leaves the menu unchanged" {
    probe() { printf '<html><a href="https://myanimelist.net/anime/44/x/">MAL</a></html>'; }
    curl_exe=probe
    anidb_desc "solo-show-44" || true
    [ "$mal_id" = "44" ]
    [ -z "${seasons_option-}" ]
}
