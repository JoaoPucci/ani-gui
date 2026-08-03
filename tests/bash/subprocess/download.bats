#!/usr/bin/env bats
#
# Tests for ani-cli's `download` (5.0).
#
# Contract:
#   - $1 = stream URL, $2 = base name (no extension), $3 = extra flags.
#   - yt-dlp when available: fragment-retrying, 16-way concurrent,
#     writing $download_dir/$2.mp4. A yt-dlp SUCCESS returns without
#     touching ffmpeg.
#   - ffmpeg otherwise (yt-dlp absent or failed): stream copy of $1
#     into the same target.
#   - 5.0 has no aria2c path and no separate subtitle fetch; every URL
#     kind goes through the same yt-dlp-then-ffmpeg chain.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    # shellcheck source=/dev/null
    load '../helpers/process_stub'
    stub_setup
    stub_command yt-dlp ffmpeg

    download_dir="$BATS_TEST_TMPDIR/dl"
    mkdir -p "$download_dir"
}

@test "download: yt-dlp present handles the url and ffmpeg stays untouched" {
    download "https://cdn.example/op/master.m3u8" "One Piece Episode 1"
    stub_assert_called yt-dlp '.*https://cdn.example/op/master.m3u8.*--no-skip-unavailable-fragments.*--fragment-retries infinite.*-N 16.*'
    stub_assert_called yt-dlp ".*-o $download_dir/One Piece Episode 1.mp4"
    stub_assert_not_called ffmpeg
}

@test "download: falls back to ffmpeg when yt-dlp is missing" {
    command() {
        if [ "$1" = "-v" ] && [ "$2" = "yt-dlp" ]; then return 1; fi
        builtin command "$@"
    }
    export -f command
    download "https://cdn.example/op/master.m3u8" "One Piece Episode 1"
    stub_assert_called ffmpeg '.*-i https://cdn.example/op/master.m3u8.*-c copy.*'
    stub_assert_called ffmpeg ".*$download_dir/One Piece Episode 1.mp4"
    stub_assert_not_called yt-dlp
}

@test "download: a failing yt-dlp run falls through to ffmpeg" {
    # The && chain returns early only on yt-dlp success; a mid-download
    # failure retries the whole stream through ffmpeg. Failing variant
    # of the stub, still logging its argv.
    yt-dlp() {
        printf 'yt-dlp %s\n' "$*" >>"$STUB_CALLS"
        return 1
    }
    export -f yt-dlp
    download "https://cdn.example/op/master.m3u8" "One Piece Episode 1"
    stub_assert_called yt-dlp '.*master.m3u8.*'
    stub_assert_called ffmpeg '.*-i https://cdn.example/op/master.m3u8.*'
}

@test "download: extra flags ride along to yt-dlp" {
    download "https://cdn.example/op/master.m3u8" "One Piece Episode 1" "--simulate"
    stub_assert_called yt-dlp '.*--simulate.*'
}
