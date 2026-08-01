#!/usr/bin/env bats
#
# Whether the wiring check can tell a suite that runs from one the
# runner merely names.
#
# The check reads `tests/bash/helpers/run-suite.sh` and asserts that
# every directory of `.bats` files is wired into it. Two readings of
# the runner's text have already been shown to be weaker than the
# property: membership in the loop's list is not execution, and a body
# that mentions the bats binary somewhere is not every suite reaching
# it. One `[ "$suite" = arch ] && continue` before the invocation
# leaves both readings intact while the arch cases stop running.
#
# So the cases below sabotage the real runner and require the check to
# notice. Each edit is verified to have changed the file, because a
# sabotage that silently does nothing reads exactly like a check that
# works — a mistake already made on this branch, which cost a working
# guard.

load '../helpers/loader'

setup() {
    CHECK="$REPO_ROOT/tests/arch/bats_suite_wiring.sh"
    RUNNER="$REPO_ROOT/tests/bash/helpers/run-suite.sh"
    SUITES="$REPO_ROOT/tests/bash"
}

# A copy of the real runner with one edit applied by the given awk
# program. Fails outright when the edit changed nothing, so a case
# cannot pass by having sabotaged an expression that is not there.
edited_runner() {
    copy="$BATS_TEST_TMPDIR/run-suite.sh"
    awk "$1" "$RUNNER" >"$copy"
    cmp -s "$copy" "$RUNNER" && return 1
    printf '%s\n' "$copy"
}

@test "the runner as it stands wires every suite" {
    run sh "$CHECK" "$RUNNER" "$SUITES"
    [ "$status" -eq 0 ]
}

@test "a suite the loop body skips is not wired" {
    # The list still names `arch` and the body still mentions the bats
    # binary, so both textual readings report healthy. Nothing in
    # `tests/bash/arch/` runs.
    copy=$(edited_runner '/^[[:space:]]*files=/ && !ins { print "    [ \"$suite\" = arch ] && continue"; ins = 1 } { print }')
    run ! sh "$CHECK" "$copy" "$SUITES"
}

@test "a runner that never reaches the bats binary is not wiring" {
    # Every suite is walked, none is run, and the loop is intact.
    copy=$(edited_runner '{ sub(/"\$BATS_BIN" \$files/, ":"); print }')
    run ! sh "$CHECK" "$copy" "$SUITES"
}

@test "a suite dropped from the loop list is not wired" {
    # The original property: a directory of cases the runner never
    # names at all.
    copy=$(edited_runner '{ sub(/ arch;/, ";"); print }')
    run ! sh "$CHECK" "$copy" "$SUITES"
}

@test "a suites directory holding nothing is reported, not passed" {
    # With no suite to require, every reading of any runner is
    # vacuously satisfied. A check that has lost its subject has to say
    # so rather than report ok.
    empty="$BATS_TEST_TMPDIR/no-suites"
    mkdir -p "$empty"
    run ! sh "$CHECK" "$RUNNER" "$empty"
}
