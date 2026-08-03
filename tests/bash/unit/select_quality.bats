#!/usr/bin/env bats
#
# Unit tests for ani-cli's `select_quality` (5.0).
#
# Contract (input via globals, output via globals):
#   Inputs:  $links (newline-separated "HEIGHTp >URL" lines, sorted
#            descending by get_video_link).
#   Output:  $video_link (the URL after the ">" of the chosen line).
#
# Quality argument:
#   - "best"  → first line.
#   - "worst" → last line starting with 3-4 digits.
#   - other   → first line containing the literal string.
#   - not found → falls back to best, warns "not found, defaulting to
#     best" on stderr.
#
# 5.0 dropped the 4.15-era side channels (refr_flag, subs_flag,
# provider referrers): the function reads $links and writes
# $video_link, nothing else.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    unset video_link
    links=$'1080p >https://cdn.example/1080/index.m3u8\n720p >https://cdn.example/720/index.m3u8\n480p >https://cdn.example/480/index.m3u8'
}

@test "select_quality: 'best' picks the first link" {
    select_quality "best" || true
    [ "$video_link" = "https://cdn.example/1080/index.m3u8" ]
}

@test "select_quality: 'worst' picks the last numeric-quality link" {
    select_quality "worst" || true
    [ "$video_link" = "https://cdn.example/480/index.m3u8" ]
}

@test "select_quality: explicit '720' picks the 720p link" {
    select_quality "720" || true
    [ "$video_link" = "https://cdn.example/720/index.m3u8" ]
}

@test "select_quality: not-found falls back to best with a stderr warning" {
    # A command substitution would run select_quality in a subshell and
    # lose the $video_link global; capture stderr via file instead.
    select_quality "9999" 2>"$BATS_TEST_TMPDIR/warn" || true
    [ "$video_link" = "https://cdn.example/1080/index.m3u8" ]
    grep -q "not found, defaulting to best" "$BATS_TEST_TMPDIR/warn"
}

@test "select_quality: 'worst' ignores non-numeric lines when picking the tail" {
    links=$'1080p >https://cdn.example/1080/index.m3u8\naudio >https://cdn.example/audio/index.m3u8'
    select_quality "worst" || true
    [ "$video_link" = "https://cdn.example/1080/index.m3u8" ]
}
