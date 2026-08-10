#!/usr/bin/env bats
#
# Acceptance tests for ani-cli's argv-driven flags that don't require
# scraping. These exercise the dispatcher loop and the simple commands
# that exit before the search/play pipeline.
#
# We run the real ./ani-cli with a sandboxed history dir; none of these
# scenarios reach the network.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    export ANI_CLI_PLAYER='debug'
}

@test "ani-cli --version prints just the version number and exits 0" {
    run "$ANI_CLI_PATH" --version
    [ "$status" -eq 0 ]
    line_count=$(printf '%s\n' "$output" | wc -l | tr -d ' ')
    [ "$line_count" -eq 1 ]
    [[ "$output" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

@test "ani-cli -V prints just the version number and exits 0 (short flag)" {
    run "$ANI_CLI_PATH" -V
    [ "$status" -eq 0 ]
    [[ "$output" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

@test "ani-cli -h prints usage information including 'Options:' and exits 0" {
    run "$ANI_CLI_PATH" -h
    [ "$status" -eq 0 ]
    [[ "$output" == *"Usage:"* ]]
    [[ "$output" == *"Options:"* ]]
    [[ "$output" == *"--continue"* ]]
    [[ "$output" == *"--quality"* ]]
    [[ "$output" == *"--vlc"* ]]
}

@test "ani-cli -D clears the history file and exits 0" {
    histfile="$ANI_CLI_HIST_DIR/ani-hsts"
    printf '2\tone-piece-69\tOne Piece\n' >"$histfile"
    [ -s "$histfile" ]
    run "$ANI_CLI_PATH" -D
    [ "$status" -eq 0 ]
    [ -f "$histfile" ]
    [ ! -s "$histfile" ]
}

@test "ANI_CLI_PLAYER pointing at an existing path is accepted at startup" {
    # 5.0 absorbed the fork's 4.15 patch: dep_ch's `command -v` prints
    # any existing slash-containing path (the Steamdeck flatpak
    # directory case), so a path player passes the dependency check.
    histfile="$ANI_CLI_HIST_DIR/ani-hsts"
    : >"$histfile"
    player_dir="$BATS_TEST_TMPDIR/flatpak-style-player-dir"
    mkdir -p "$player_dir"
    export ANI_CLI_PLAYER="$player_dir/"
    run "$ANI_CLI_PATH" -c
    [ "$status" -eq 1 ]
    [[ "$output" != *"not found"* ]]
    [[ "$output" == *"No unwatched series in history"* ]]
}

@test "ANI_CLI_PLAYER pointing at a missing path dies at the dependency check" {
    # The complement: 5.0 accepts a path player only when it exists.
    # (4.15 needed a fork patch to accept the flatpak forms at all;
    # the alias spelling `flatpak_mpv` retired with that patch — 5.0
    # documents only the directory default.)
    histfile="$ANI_CLI_HIST_DIR/ani-hsts"
    : >"$histfile"
    export ANI_CLI_PLAYER="$BATS_TEST_TMPDIR/does-not-exist/player/"
    run "$ANI_CLI_PATH" -c
    [ "$status" -eq 1 ]
    [[ "$output" == *"not found"* ]]
}

@test "ani-cli -c with empty history dies 'No unwatched series in history!'" {
    histfile="$ANI_CLI_HIST_DIR/ani-hsts"
    : >"$histfile"
    run "$ANI_CLI_PATH" -c
    [ "$status" -eq 1 ]
    [[ "$output" == *"No unwatched series in history"* ]]
}
