#!/usr/bin/env bats
#
# Tests for ani-cli's `anidb_m3u8` + `get_video_link` (5.0's stream
# resolution: languages endpoint → embed page → master playlist).
#
# Contract — anidb_m3u8 <ep_no> <mode>:
#   - Resolves <ep_no> to the episode db id through $episode_maps.
#   - GETs /api/frontend/episode/<id>/languages; picks the first
#     embed_url whose entry mentions "jpn" (sub) or "eng" (dub).
#   - GETs the embed page; extracts the master-playlist URL from the
#     jwplayer `file: '…'` assignment.
#   - GETs the master playlist; emits "HEIGHTp >variant-url" lines.
#   - Returns 1 without fetching further when no embed matches.
#
# Contract — get_video_link:
#   - Sorts anidb_m3u8's lines descending, dies "No sources found"
#     when empty, then select_quality picks $video_link.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    export CURL_FIXTURE_DIR="$FIXTURES_DIR/anidb"
    curl_exe="$REPO_ROOT/tests/bash/helpers/curl_shim.sh"
    episode_maps="9001"$'\t'"1"$'\n'"9002"$'\t'"2"$'\n'"9003"$'\t'"3"
}

@test "anidb_m3u8: resolves an episode to quality-tagged variant links" {
    output=$(anidb_m3u8 "1" "sub")
    printf '%s\n' "$output" | grep -F "1080p >https://cdn.example/op/1080/index.m3u8" >/dev/null
    printf '%s\n' "$output" | grep -F "720p >https://cdn.example/op/720/index.m3u8" >/dev/null
}

@test "anidb_m3u8: dub mode selects the eng embed" {
    # Fetches run inside command substitutions, so the probe records
    # embed hits through files rather than variables.
    probe() {
        case "$*" in
            */languages*) cat "$FIXTURES_DIR/anidb/languages_op.json" ;;
            *op-eng*) touch "$BATS_TEST_TMPDIR/hit-eng" && cat "$FIXTURES_DIR/anidb/embed_op.html" ;;
            *op-jpn*) touch "$BATS_TEST_TMPDIR/hit-jpn" && cat "$FIXTURES_DIR/anidb/embed_op.html" ;;
            *) cat "$FIXTURES_DIR/anidb/master_op.m3u8" ;;
        esac
    }
    curl_exe=probe
    anidb_m3u8 "1" "dub" >/dev/null
    [ -f "$BATS_TEST_TMPDIR/hit-eng" ]
    [ ! -f "$BATS_TEST_TMPDIR/hit-jpn" ]
}

@test "anidb_m3u8: returns 1 without an embed fetch when no language matches" {
    probe() {
        case "$*" in
            */languages*) printf '[]' ;;
            *) touch "$BATS_TEST_TMPDIR/fetched-embed" ;;
        esac
    }
    curl_exe=probe
    run anidb_m3u8 "1" "sub"
    [ "$status" -eq 1 ]
    [ ! -f "$BATS_TEST_TMPDIR/fetched-embed" ]
}

@test "anidb_m3u8: an unmapped episode number resolves nothing" {
    # The probe answers only the mapped id: an unmapped episode makes
    # the languages URL with an empty id, which no catalogue serves.
    # (The PATH shim's glob routing is looser than a real server and
    # would match the malformed URL too.)
    probe() {
        case "$*" in
            */api/frontend/episode/9001/languages*) cat "$FIXTURES_DIR/anidb/languages_op.json" ;;
            *) printf '[]' ;;
        esac
    }
    curl_exe=probe
    episode_maps="9001"$'\t'"1"
    run anidb_m3u8 "7" "sub"
    [ "$status" -eq 1 ]
}

@test "get_video_link: picks the best variant into video_link" {
    ep_no='1'
    mode='sub'
    quality='best'
    ep_list=$'1\n2\n3'
    get_video_link 2>/dev/null || true
    [ "$video_link" = "https://cdn.example/op/1080/index.m3u8" ]
}

@test "get_video_link: dies 'No sources found' when the resolver comes back empty" {
    probe() { printf '[]'; }
    curl_exe=probe
    ep_no=1 mode=sub quality=best ep_list=1
    run get_video_link
    [ "$status" -eq 1 ]
    [[ "$output" == *"No sources found"* ]]
}
