#!/usr/bin/env bats
#
# Unit tests for ani-cli's `dep_ch` and `dep_ch_failover` (lines 124-136).
#
# Contract — dep_ch (since 4.15 a single-dependency check):
#   - Takes ONE dependency name; runs `command -v` on it verbatim.
#   - Extra positional args are ignored (callers pass one name per call).
#   - If absent, calls `die` (printf to stderr + exit 1).
#   - If present, returns 0 with no output.
#
# Contract — dep_ch_failover (new in 4.15; replaces where_iina/where_mpv):
#   - Takes ONE comma-separated candidate list.
#   - Prints the first candidate that resolves via `command -v` OR exists
#     as a filesystem path (`-e`), returns 0.
#   - Returns 1 (no output) when no candidate resolves or the list is
#     empty. ani-cli uses this for the player fallback chain, the menu
#     helper (fzf/rofi/dmenu), and the hard botan requirement.

load '../helpers/loader'

@test "dep_ch: returns 0 when the dep is present" {
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch bash'
    [ "$status" -eq 0 ]
    [ -z "$output" ]
}

@test "dep_ch: dies with exit 1 when the dep is missing" {
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch __definitely_not_a_real_cmd__'
    [ "$status" -eq 1 ]
    [[ "$output" =~ "not found" ]]
    [[ "$output" =~ "__definitely_not_a_real_cmd__" ]]
}

@test "dep_ch: checks only the first arg — extra args are ignored" {
    # 4.14.5 looped over every arg; since 4.15 only $1 is inspected.
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch bash __also_missing__'
    [ "$status" -eq 0 ]
    [ -z "$output" ]
}

@test "dep_ch: no word-splitting — a name with a flag is one lookup and dies" {
    # 4.14.5 checked the first whitespace-separated word; since 4.15 the
    # whole string is looked up verbatim, so "bash --some-flag" is not a
    # resolvable command.
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch "bash --some-flag"'
    [ "$status" -eq 1 ]
    [[ "$output" =~ "not found" ]]
}

@test "dep_ch_failover: prints the first present candidate" {
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch_failover "bash,__nope__"'
    [ "$status" -eq 0 ]
    [ "$output" = "bash" ]
}

@test "dep_ch_failover: falls through missing candidates to a later one" {
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch_failover "__nope__,__still_nope__,bash"'
    [ "$status" -eq 0 ]
    [ "$output" = "bash" ]
}

@test "dep_ch_failover: accepts an existing filesystem path that is not a command" {
    # The Linux player chain lists a flatpak directory path; `-e` matches
    # it even though `command -v` cannot.
    marker="$BATS_TEST_TMPDIR/not_on_path_marker"
    touch "$marker"
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch_failover "'"$marker"',bash"'
    [ "$status" -eq 0 ]
    [ "$output" = "$marker" ]
}

@test "dep_ch_failover: returns 1 with no output when nothing resolves" {
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch_failover "__nope__,__still_nope__"'
    [ "$status" -eq 1 ]
    [ -z "$output" ]
}

@test "dep_ch_failover: empty input returns 1" {
    run bash -c '__ANI_CLI_LIB__=1 . "'"$ANI_CLI_PATH"'" 2>/dev/null; dep_ch_failover ""'
    [ "$status" -eq 1 ]
    [ -z "$output" ]
}
