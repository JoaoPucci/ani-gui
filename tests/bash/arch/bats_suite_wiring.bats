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
    copy=$(edited_runner '{ sub(/"\$BATS_BIN" "\$@"/, ":"); print }')
    run ! sh "$CHECK" "$copy" "$SUITES"
}

@test "a suite dropped from the loop list is not wired" {
    # The original property: a directory of cases the runner never
    # names at all.
    copy=$(edited_runner '{ sub(/ arch;/, ";"); print }')
    run ! sh "$CHECK" "$copy" "$SUITES"
}

@test "a scratch path that already exists is left alone" {
    # `<tmpdir>/ani-gui-bats-wiring.$$` is predictable, and a pid comes
    # round again after a SIGKILL that skipped cleanup. The trap is
    # armed before the directory is created — deliberately, so no
    # signal can leave one behind — which means a failed `mkdir` would
    # otherwise hand somebody else's directory to `rm -rf`.
    occupied="$BATS_TEST_TMPDIR/occupied"
    mkdir -p "$occupied"
    printf 'not yours\n' >"$occupied/keep-me"
    run ! env ARCH_WIRING_SCRATCH="$occupied" sh "$CHECK" "$RUNNER" "$SUITES"
    [ -f "$occupied/keep-me" ]
}

@test "a scratch path containing a quote does not break the stub" {
    # The stub is generated, and the record path goes into it. Spelled
    # into the program text, a path holding a double quote closes a
    # string the program opened and every suite invocation dies of
    # syntax — reported as suites that never reached the binary, which
    # is a true statement about a cause that has nothing to do with the
    # runner.
    # Driven through `TMPDIR`, which the check already reads, so the
    # case bites against the code as it stands rather than against a
    # seam introduced to receive it.
    #
    # A quote and no whitespace. The runner splits `$files` on purpose
    # and so cannot carry a suite path containing a space — its own
    # limitation, and including one here would fail this case for a
    # reason that has nothing to do with the stub.
    quoted="$BATS_TEST_TMPDIR/say\"hi\""
    mkdir -p "$quoted"
    run env TMPDIR="$quoted" sh "$CHECK" "$RUNNER" "$SUITES"
    [ "$status" -eq 0 ]
}

@test "a runner that takes only one file per suite is not wiring" {
    # The sandbox mirrors one probe per directory, so a runner that
    # keeps the directory and drops all but the first file satisfies a
    # per-directory reading exactly. Most of the arch suite would stop
    # running and this check would say every suite reached the binary.
    copy=$(edited_runner '{ sub(/\| sort\)/, "| sort | head -1)"); print }')
    run ! sh "$CHECK" "$copy" "$SUITES"
}

@test "a signal before the claim leaves a stranger's directory alone" {
    # The traps are armed before `mkdir`, deliberately, so that no
    # signal can leave a directory behind. That leaves a window in
    # which they are armed and this run owns nothing: another process
    # taking the path in that instant has it removed by a handler
    # acting on behalf of a run that never created it.
    #
    # The window is too short to step into, so the check holds it open
    # on request. Everything else here is ordinary.
    target="$BATS_TEST_TMPDIR/window"
    env ARCH_WIRING_SCRATCH="$target" ARCH_WIRING_PAUSE_BEFORE_MKDIR=3 \
        sh "$CHECK" "$RUNNER" "$SUITES" >/dev/null 2>&1 &
    pid=$!
    sleep 1
    mkdir -p "$target"
    printf 'not yours\n' >"$target/keep-me"
    kill -TERM "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    [ -f "$target/keep-me" ]
}

@test "a path containing a space does not stop the suites running" {
    # The runner word-split its file list, so a checkout under
    # `~/My Repos/` could not run its own tests: every filename arrived
    # at bats in pieces. This check builds a sandbox under `TMPDIR`, so
    # it is the place that notices.
    spaced="$BATS_TEST_TMPDIR/a b"
    mkdir -p "$spaced"
    run env TMPDIR="$spaced" sh "$CHECK" "$RUNNER" "$SUITES"
    [ "$status" -eq 0 ]
}

@test "a suites directory holding nothing is reported, not passed" {
    # With no suite to require, every reading of any runner is
    # vacuously satisfied. A check that has lost its subject has to say
    # so rather than report ok.
    empty="$BATS_TEST_TMPDIR/no-suites"
    mkdir -p "$empty"
    run ! sh "$CHECK" "$RUNNER" "$empty"
}
