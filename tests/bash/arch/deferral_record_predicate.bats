#!/usr/bin/env bats
#
# `record_is_recoverable` — whether a declared record survives a fresh
# clone.
#
# "Recoverable" means tracked, because that is the only state a clone
# reconstructs. A file present on someone's disk but never added is
# indistinguishable from an absent one to anybody else, and an ignored
# path is the defect this invariant exists for: policy that names a
# ledger nobody else can read records nothing.

load '../helpers/loader'

setup() {
    export __DEFERRAL_RECORD_LIB__=1
    export ARCH_REPO_ROOT="$REPO_ROOT"
    # shellcheck disable=SC1091
    . "$REPO_ROOT/tests/arch/deferral_record.sh"
}

@test "the declared ledger is recoverable" {
    # The path AGENTS.md §14 names. It has to be tracked here, because
    # both alternatives are closed: the internal planning directory is
    # git-ignored, and this repository has issues disabled.
    record_is_recoverable docs/deferred-work.md
}

@test "an ordinary tracked file is recoverable" {
    record_is_recoverable tests/arch/run-all.sh
}

@test "an ignored path is rejected" {
    # The defect the invariant was written for.
    run ! record_is_recoverable .planning/follow-ups.md
}

@test "a name git reads as an option is rejected" {
    # `--stage` is a real `ls-files` flag, so an unguarded query
    # succeeds by listing the whole index and the absent record reads
    # as present.
    run ! record_is_recoverable '--stage'
}

@test "a glob matching a tracked file is rejected" {
    # `[A]GENTS.md` matches the tracked `AGENTS.md` as a pathspec while
    # the literal file does not exist, so the record would be reported
    # readable because something else is.
    run ! record_is_recoverable '[A]GENTS.md'
}

@test "an absent path is rejected" {
    # The case an ignore-only check waves through: nothing ignores it,
    # and it is still not there.
    run ! record_is_recoverable docs/no-such-follow-ups.md
}

@test "a file present but never added is rejected" {
    # Same consequence as absent — a clone rebuilds from the index,
    # not from someone's disk.
    # Inside a repository, not beside one. A probe under
    # $BATS_TEST_TMPDIR is outside the checkout entirely, so `git
    # ls-files` rejects it as an external path and the case passes
    # without ever meeting a file that exists in a working tree and is
    # absent from its index — which is the state being asserted.
    repo="$BATS_TEST_TMPDIR/untracked-repo"
    mkdir -p "$repo"
    (cd "$repo" && git init -q .) >/dev/null 2>&1
    probe=$(make_untracked_probe "$repo")
    [ -e "$probe" ]
    run ! env ARCH_REPO_ROOT="$repo" sh -c '
        __DEFERRAL_RECORD_LIB__=1
        . "$1/tests/arch/deferral_record.sh"
        cd "$2" || exit 1
        record_is_recoverable "${3##*/}"
    ' _ "$REPO_ROOT" "$repo" "$probe"
}

@test "the untracked probe never reuses or truncates a path" {
    # A fixed probe name would truncate a developer's own file on the
    # way in and delete it on the way out, so the suite would destroy
    # unrelated local work as a side effect. Demonstrated without
    # naming a path of our own: the first probe becomes the sentinel,
    # and the second has to leave it alone.
    first=$(make_untracked_probe "$BATS_TEST_TMPDIR")
    printf 'do not lose me\n' >"$first"
    second=$(make_untracked_probe "$BATS_TEST_TMPDIR")
    [ "$second" != "$first" ]
    [ "$(cat "$first")" = 'do not lose me' ]
}
