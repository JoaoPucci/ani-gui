#!/usr/bin/env bats
#
# An index entry is not the same as a readable record.
#
# A symlink and a gitlink both get one, and neither carries content:
# the link may point outside the repository or at nothing, and a
# submodule is an empty directory until someone initialises it. Either
# would have the check report a durable record where a fresh clone
# finds none.
#
# Driven against a scratch repository, because the entries have to
# exist in an index and this repository is not going to grow a symlink
# for the sake of a test. No commit is needed — `ls-files` reads the
# index.

load '../helpers/loader'

setup() {
    export ARCH_DEFERRAL_RECORD_LIB=1
    export ARCH_REPO_ROOT="$REPO_ROOT"
    # shellcheck disable=SC1091
    . "$REPO_ROOT/tests/arch/deferral_record.sh"

    LINKREPO="$BATS_TEST_TMPDIR/linkrepo"
    mkdir -p "$LINKREPO"
    (
        cd "$LINKREPO" || exit 1
        git init -q .
        printf 'content\n' >real.md
        ln -s /etc/hostname outside.md
        ln -s nowhere.md broken.md
        mkdir -p mixed allgood
        printf 'content\n' >mixed/content.md
        ln -s /etc/hostname mixed/outside.md
        printf 'content\n' >allgood/content.md
        git add real.md outside.md broken.md mixed allgood
    ) >/dev/null 2>&1
}

# The predicate resolves paths against the repository it was pointed
# at, so each case runs from inside the scratch one.
in_linkrepo() { (cd "$LINKREPO" && record_is_recoverable "$1"); }
# bats runs each test in its own subshell, so a `cd` here cannot leak.

@test "a tracked regular file is a record" {
    in_linkrepo real.md
}

@test "a tracked symlink pointing outside the repository is rejected" {
    run ! in_linkrepo outside.md
}

@test "a tracked symlink pointing at nothing is rejected" {
    run ! in_linkrepo broken.md
}

@test "a directory of regular files is a record" {
    in_linkrepo allgood/
}

@test "a directory is rejected when any entry is a link" {
    # A declared directory is one record, so every entry under it has
    # to carry content. One regular file is enough to satisfy a check
    # that stops at the first match, and the clone still arrives with
    # a link pointing nowhere.
    run ! in_linkrepo mixed/
}

@test "a tracked symlink is reported as the wrong kind of entry" {
    # The reason has to match the fix. A tracked symlink is tracked, so
    # telling its author it "is not tracked by git" sends them to `git
    # add` a path git already has — the one action that cannot help.
    reason=$(cd "$LINKREPO" && why_unrecoverable outside.md)
    [ "$reason" != 'which is not tracked by git' ]
}

@test "an ignored record is reported as ignored, not merely untracked" {
    # "Ignored" and "not tracked" call for different fixes — unignore
    # it, or add it — and a check reporting the wrong one sends the
    # reader the wrong way.
    [ "$(why_unrecoverable '.planning/follow-ups.md')" = 'which git ignores' ]
}

@test "an absent record is reported as untracked" {
    [ "$(why_unrecoverable 'docs/no-such-follow-ups.md')" = 'which is not tracked by git' ]
}

@test "an intent-to-add record is not carried" {
    # `git add -N` records only the intent. The entry is in the index
    # with a regular mode and the empty blob, so every mode test
    # passes — but `write-tree` omits the path, so the next commit and
    # every clone from it lack the record entirely.
    ita="$BATS_TEST_TMPDIR/intent-to-add"
    mkdir -p "$ita"
    (
        cd "$ita" || exit 1
        git init -q .
        printf 'log\n' >LEDGER.md
        git add -N LEDGER.md
    ) >/dev/null 2>&1
    cd "$ita" || return 1
    run ! record_is_recoverable LEDGER.md
}
