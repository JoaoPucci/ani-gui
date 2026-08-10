#!/usr/bin/env bats
#
# Tests for ani-cli's `process_hist_entry` (5.0).
#
# For one history row the function asks the provider for the episode
# list and prints "id\ttitle - episode N" where N is the episode AFTER
# the recorded one — the row a continue-watching menu offers. A show
# watched to its last known episode yields nothing and drops out of
# the menu. The provider lookup is a seam: `anidb_episodes` is
# overridden with canned rows, the way the network suites substitute
# curl, so these cases exercise only the pure next-episode selection.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    # Defined after the sourcing, or the real curl-backed function
    # from ani-cli replaces the stub and every case goes vacuous.
    anidb_episodes() {
        printf 'one-piece-69\t1\n'
        printf 'one-piece-69\t2\n'
        printf 'one-piece-69\t3\n'
    }
}

@test "process_hist_entry: offers the episode after the recorded one" {
    anime_id="one-piece-69" anime_title="One Piece" ep_no=2
    run process_hist_entry
    assert_output "$(printf 'one-piece-69\tOne Piece - episode 3')"
}

@test "process_hist_entry: a show at its last known episode yields nothing" {
    anime_id="one-piece-69" anime_title="One Piece" ep_no=3
    run process_hist_entry
    assert_output ""
}

@test "process_hist_entry: an episode missing from the list yields nothing" {
    anime_id="one-piece-69" anime_title="One Piece" ep_no=9
    run process_hist_entry
    assert_output ""
}
