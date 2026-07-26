#!/usr/bin/env bats
#
# Acceptance tests for ani-cli's argv-driven flags that don't require
# scraping. These exercise the dispatcher loop and the simple commands
# that exit before the search/play pipeline.
#
# We run the real ./ani-cli with all needed env vars and stubbed PATH
# entries (curl, mpv) — but for these scenarios none of those external
# commands are actually invoked.

load '../helpers/loader'

setup() {
    # History sandbox so -D / -c can't touch the user's real history.
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    export ANI_CLI_PLAYER='debug'

    # ani-cli 4.15 hard-requires a botan binary at startup (dep_ch_failover
    # before the search dispatch), so even no-network flows like -c need
    # one on PATH. The -V/-h/-D flows exit during arg parsing and never
    # reach the check; the shim is harmless there.
    export PATH_SHIM="$BATS_TEST_TMPDIR/bin"
    mkdir -p "$PATH_SHIM"
    cp "$REPO_ROOT/tests/bash/helpers/fake_botan.sh" "$PATH_SHIM/botan"
    chmod +x "$PATH_SHIM/botan"
    export PATH="$PATH_SHIM:$PATH"
}

@test "ani-cli --version prints just the version number and exits 0" {
    run "$ANI_CLI_PATH" --version
    [ "$status" -eq 0 ]
    # The version line is the only stdout content.
    line_count=$(printf '%s\n' "$output" | wc -l | tr -d ' ')
    [ "$line_count" -eq 1 ]
    # Looks like a semver triple: <major>.<minor>.<patch>
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
    cp "$FIXTURES_DIR/history/multi.tsv" "$histfile"
    [ -s "$histfile" ]
    run "$ANI_CLI_PATH" -D
    [ "$status" -eq 0 ]
    # File still exists but is now empty.
    [ -f "$histfile" ]
    [ ! -s "$histfile" ]
}

@test "ani-cli -c with empty history dies 'No unwatched series in history!'" {
    histfile="$ANI_CLI_HIST_DIR/ani-hsts"
    : >"$histfile"
    run "$ANI_CLI_PATH" -c
    [ "$status" -eq 1 ]
    [[ "$output" == *"No unwatched series in history"* ]]
}
