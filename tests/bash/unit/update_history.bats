#!/usr/bin/env bats
#
# Tests for ani-cli's `update_history` (5.0).
#
# History rows are "ep\tid\ttitle". A watched episode either rewrites
# the row already keyed on the anidb slug or appends a first row for a
# show the file has never seen. The rewrite goes through
# ${histfile}.new and lands atomically via mv, and the replacement
# passes the title through a sed escape so the characters sed reads as
# replacement syntax (&, \, and the | delimiter) arrive in the file
# literally. The GUI parses this same file for its history rail, so
# the shapes pinned here are what its parser will meet.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    histfile="$BATS_TEST_TMPDIR/ani-hsts"
}

@test "update_history: appends a first row for an unseen show" {
    printf '2\tone-piece-69\tOne Piece\n' >"$histfile"
    ep_no=1 anime_id="naruto-13" anime_title="Naruto"
    update_history
    run cat "$histfile"
    assert_line --index 0 "$(printf '2\tone-piece-69\tOne Piece')"
    assert_line --index 1 "$(printf '1\tnaruto-13\tNaruto')"
}

@test "update_history: rewrites the row for a show already on file" {
    {
        printf '2\tone-piece-69\tOne Piece\n'
        printf '5\tnaruto-13\tNaruto\n'
    } >"$histfile"
    ep_no=6 anime_id="naruto-13" anime_title="Naruto"
    update_history
    run cat "$histfile"
    assert_line --index 0 "$(printf '2\tone-piece-69\tOne Piece')"
    assert_line --index 1 "$(printf '6\tnaruto-13\tNaruto')"
    [ "${#lines[@]}" -eq 2 ]
}

@test "update_history: a title holding sed replacement syntax lands literally" {
    printf '1\tzero-2\tRe&Zero \\ Part | Two\n' >"$histfile"
    ep_no=2 anime_id="zero-2" anime_title='Re&Zero \ Part | Two'
    update_history
    run cat "$histfile"
    assert_line --index 0 "$(printf '2\tzero-2\tRe&Zero \\ Part | Two')"
    [ "${#lines[@]}" -eq 1 ]
}

@test "update_history: consumes the temp rewrite file" {
    printf '1\tone-piece-69\tOne Piece\n' >"$histfile"
    ep_no=2 anime_id="one-piece-69" anime_title="One Piece"
    update_history
    [ ! -e "${histfile}.new" ]
}
