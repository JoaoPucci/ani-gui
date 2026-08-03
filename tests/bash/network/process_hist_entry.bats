#!/usr/bin/env bats
#
# Tests for ani-cli's `process_hist_entry` (5.0).
#
# Contract:
#   - Reads $anime_id, $anime_title, $ep_no from caller (one history
#     line).
#   - Calls anidb_episodes "$anime_id" and keeps the episode-number
#     column ($ep_list).
#   - Takes the episode AFTER the current ep_no.
#   - If a next episode exists, prints "anime_id\ttitle - episode N".
#   - Otherwise prints nothing: 5.0 drops caught-up shows from the -c
#     list. (4.15 kept them via an "(up to date)" row, and the fork
#     carried a guard on that fallback; both retired with the
#     provider.)
#
# anidb_episodes is mocked as a function override so this stays
# hermetic.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    anidb_episodes() {
        printf '9001\t1\n9002\t2\n9003\t3\n9004\t4\n9005\t5\n'
    }
}

@test "process_hist_entry: emits the next episode when more are available" {
    anime_id='one-piece-69'
    anime_title='One Piece'
    ep_no='2'
    output=$(process_hist_entry || true)
    expected="one-piece-69"$'\t'"One Piece - episode 3"
    [ "$output" = "$expected" ]
}

@test "process_hist_entry: emits nothing at the last episode (caught-up shows drop)" {
    anime_id='one-piece-69'
    anime_title='One Piece'
    ep_no='5'
    output=$(process_hist_entry || true)
    [ -z "$output" ]
}

@test "process_hist_entry: emits nothing when the episode fetch returns no rows" {
    anidb_episodes() { :; }
    anime_id='one-piece-69'
    anime_title='One Piece'
    ep_no='2'
    output=$(process_hist_entry || true)
    [ -z "$output" ]
}

@test "process_hist_entry: emits nothing when the saved episode left the list" {
    anime_id='one-piece-69'
    anime_title='One Piece'
    ep_no='9'
    output=$(process_hist_entry || true)
    [ -z "$output" ]
}
