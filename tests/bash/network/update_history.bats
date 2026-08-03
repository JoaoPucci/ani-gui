#!/usr/bin/env bats
#
# Tests for ani-cli's `update_history` (5.0).
#
# Contract:
#   - If $anime_id is found in $histfile, rewrite that row as
#     "ep_no\tanime_id\ttitle" (the title's sed specials & \ | are
#     escaped first).
#   - Else, append a new "ep_no\tanime_id\ttitle" row.
#   - Writes atomically via $histfile.new + mv.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    histfile="$BATS_TEST_TMPDIR/ani-hsts"
    : >"$histfile"
}

@test "update_history: appends a new entry when the id is absent" {
    printf '5\tattack-on-titan-919\tAttack on Titan\n' >"$histfile"
    anime_id='one-piece-69'
    ep_no='1'
    anime_title='One Piece'
    update_history
    line_count=$(wc -l <"$histfile" | tr -d ' ')
    [ "$line_count" -eq 2 ]
    grep -F "1"$'\t'"one-piece-69"$'\t'"One Piece" "$histfile" >/dev/null
    grep -F "5"$'\t'"attack-on-titan-919"$'\t'"Attack on Titan" "$histfile" >/dev/null
}

@test "update_history: updates ep_no on the matching id row" {
    {
        printf '12\tattack-on-titan-919\tAttack on Titan\n'
        printf '3\tdemon-slayer-101\tDemon Slayer\n'
        printf '1\tspy-x-family-303\tSpy x Family\n'
    } >"$histfile"
    anime_id='demon-slayer-101'
    ep_no='4'
    anime_title='Demon Slayer'
    update_history
    grep -F "4"$'\t'"demon-slayer-101"$'\t'"Demon Slayer" "$histfile" >/dev/null
    grep -F "12"$'\t'"attack-on-titan-919" "$histfile" >/dev/null
    grep -F "1"$'\t'"spy-x-family-303" "$histfile" >/dev/null
    line_count=$(wc -l <"$histfile" | tr -d ' ')
    [ "$line_count" -eq 3 ]
}

@test "update_history: appending to empty histfile creates one entry" {
    : >"$histfile"
    anime_id='one-piece-69'
    ep_no='1'
    anime_title='One Piece'
    update_history
    line_count=$(wc -l <"$histfile" | tr -d ' ')
    [ "$line_count" -eq 1 ]
    grep -F "1"$'\t'"one-piece-69"$'\t'"One Piece" "$histfile" >/dev/null
}

@test "update_history: escapes sed specials in the replacement title" {
    # An unescaped & in the replacement expands to the whole matched
    # row; 5.0 escapes & \ | through safe_title before the rewrite.
    printf '1\tfoo-bar-77\tFoo & Bar\n' >"$histfile"
    anime_id='foo-bar-77'
    ep_no='2'
    anime_title='Foo & Bar'
    update_history
    grep -F "2"$'\t'"foo-bar-77"$'\t'"Foo & Bar" "$histfile" >/dev/null
    line_count=$(wc -l <"$histfile" | tr -d ' ')
    [ "$line_count" -eq 1 ]
}

@test "update_history: leaves no .new sidecar after the atomic move" {
    printf '5\tattack-on-titan-919\tAttack on Titan\n' >"$histfile"
    anime_id='attack-on-titan-919'
    ep_no='6'
    anime_title='Attack on Titan'
    update_history
    [ ! -f "${histfile}.new" ]
    [ -f "$histfile" ]
}
