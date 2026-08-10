#!/usr/bin/env bats
#
# Tests for ani-cli's `cleanup` (5.0).
#
# The EXIT handler resets terminal colors and removes the history
# rewrite temp file a signalled run may have left mid-update. The
# terminal reset is stubbed — no terminal owns this test — so the
# cases prove the temp file, and only the temp file, is taken.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    histfile="$BATS_TEST_TMPDIR/ani-hsts"
}

tput() { :; }

@test "cleanup: removes a leftover history rewrite file" {
    printf '1\tone-piece-69\tOne Piece\n' >"$histfile"
    : >"${histfile}.new"
    cleanup
    [ ! -e "${histfile}.new" ]
    [ -f "$histfile" ]
}

@test "cleanup: is safe when no rewrite file exists" {
    cleanup
    [ ! -e "${histfile}.new" ]
}
