#!/usr/bin/env bats
#
# Unit tests for ani-cli's `nth` (5.0).
#
# Contract:
#   - Reads all of stdin into a buffer.
#   - Empty stdin → returns 1 with no output.
#   - Single-line stdin → outputs `cut -f2,3` of that line, returns 0.
#   - Multi-line stdin → pipes "field1 field3" display rows through
#     `menu`, takes the first word of the pick, and emits `cut -f2,3`
#     of the matching stdin row.
#   - A multi-select pick whose first and last rows differ becomes a
#     sed range over the raw stdin (the episode-range path).
#   - An empty pick returns 1.
#
# `menu` goes to fzf/rofi/dmenu in production; each multi-line test
# overrides it inline.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
}

@test "nth: empty stdin returns 1 with no output" {
    status=0
    output=$(printf "" | nth "select" 2>&1) || status=$?
    [ "$status" -eq 1 ]
    [ -z "$output" ]
}

@test "nth: single-line stdin outputs cut -f2,3 of the line" {
    output=$(printf '1\tone-piece-69\tOne Piece\n' | nth "select")
    [ "$output" = "one-piece-69"$'\t'"One Piece" ]
}

@test "nth: single-line stdin with only two fields outputs field 2 alone" {
    output=$(printf '1\tone-piece-69\n' | nth "select")
    [ "$output" = "one-piece-69" ]
}

@test "nth: multi-line stdin resolves the menu pick back to id and title" {
    menu() { sed -n '2p'; }
    output=$(printf '1\ta-1\tAlpha\n2\tb-2\tBeta\n3\tc-3\tGamma\n' | nth "select")
    [ "$output" = "b-2"$'\t'"Beta" ]
}

@test "nth: multi-select over a bare episode list expands to the sed range" {
    # ep_list input has no tabs: field1 IS the line. A pick spanning
    # rows 2-4 must come back as the raw lines 2..4.
    menu() { printf '2\n4\n'; }
    output=$(printf '1\n2\n3\n4\n5\n' | nth "select" "-m")
    [ "$output" = $'2\n3\n4' ]
}

@test "nth: empty menu pick returns 1" {
    run --separate-stderr bash -c '
        __ANI_CLI_LIB__=1 . "$ANI_CLI_PATH" 2>/dev/null
        menu() { :; }
        printf "1\ta-1\tAlpha\n2\tb-2\tBeta\n" | nth "select"
    '
    [ "$status" -eq 1 ]
}
