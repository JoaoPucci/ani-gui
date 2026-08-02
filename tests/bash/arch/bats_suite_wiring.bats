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

# The window scenario: hold the check open before its `mkdir`, take
# the path from under it, TERM it, and require the stranger's file to
# survive. A helper rather than inline, so a case can also assert what
# happens when the timing does not hold.
window_scenario() {
    _pause=$1
    _target=$2
    env ARCH_WIRING_SCRATCH="$_target" ARCH_WIRING_PAUSE_BEFORE_MKDIR="$_pause" \
        sh "$CHECK" "$RUNNER" "$SUITES" >/dev/null 2>&1 &
    _pid=$!
    sleep 1
    # Still inside the window, or there is no window: a child that
    # already exited was never signalled, the sentinel survives
    # trivially, and success would describe cleanup nobody watched.
    kill -0 "$_pid" 2>/dev/null || {
        wait "$_pid" 2>/dev/null || true
        echo "window child exited before the path was taken"
        return 1
    }
    mkdir -p "$_target"
    printf 'not yours\n' >"$_target/keep-me"
    kill -TERM "$_pid" 2>/dev/null || true
    _status=0
    wait "$_pid" 2>/dev/null || _status=$?
    # 143 exactly: anything else means the child died of something
    # other than the signal this scenario delivers.
    [ "$_status" -eq 143 ] || {
        echo "window child exited $_status, not 143"
        return 1
    }
    [ -f "$_target/keep-me" ]
}

@test "a signal before the claim leaves a stranger's directory alone" {
    # The traps are armed before `mkdir`, deliberately, so that no
    # signal can leave a directory behind. That leaves a window in
    # which they are armed and this run owns nothing: another process
    # taking the path in that instant has it removed by a handler
    # acting on behalf of a run that never created it.
    window_scenario 3 "$BATS_TEST_TMPDIR/window"
}

@test "the window case refuses a child that raced through" {
    # With no pause the child finishes long before the parent takes
    # the path: nothing is signalled, nothing exercises the window,
    # and the sentinel survives trivially. A scenario that cannot tell
    # this from the real thing reports on cleanup it never watched.
    run ! window_scenario 0 "$BATS_TEST_TMPDIR/window-raced"
}

# The swap scenario: let the check create its scratch, hold it open,
# replace the directory with somebody else's, TERM it, and require the
# replacement to survive. `owned` records that this run created a
# directory at the path — not which directory — so removal gated on it
# alone acts on whatever sits there by exit.
swap_scenario() {
    _pause=$1
    _target=$2
    env ARCH_WIRING_SCRATCH="$_target" ARCH_WIRING_PAUSE_AFTER_MKDIR="$_pause" \
        sh "$CHECK" "$RUNNER" "$SUITES" >/dev/null 2>&1 &
    _pid=$!
    # The scratch has to be seen to exist first: replacing a directory
    # the check never made exercises nothing.
    _seen=0
    for _ in $(seq 1 50); do
        [ -d "$_target" ] && {
            _seen=1
            break
        }
        kill -0 "$_pid" 2>/dev/null || break
        sleep 0.1
    done
    [ "$_seen" -eq 1 ] || {
        wait "$_pid" 2>/dev/null || true
        echo "the check never created $_target"
        return 1
    }
    kill -0 "$_pid" 2>/dev/null || {
        wait "$_pid" 2>/dev/null || true
        echo "swap child exited before the path was swapped"
        return 1
    }
    rm -rf "$_target"
    mkdir -p "$_target"
    printf 'not yours\n' >"$_target/keep-me"
    kill -TERM "$_pid" 2>/dev/null || true
    _status=0
    wait "$_pid" 2>/dev/null || _status=$?
    [ "$_status" -eq 143 ] || {
        echo "swap child exited $_status, not 143"
        return 1
    }
    [ -f "$_target/keep-me" ]
}

@test "cleanup leaves a replaced scratch directory alone" {
    # Another process can remove the predictable path after this check
    # creates it and put its own directory there before exit. A cleanup
    # that only remembers "this run created something at the path"
    # removes the replacement on behalf of a directory that is gone.
    swap_scenario 3 "$BATS_TEST_TMPDIR/swap"
}

@test "a swap during cleanup's decision window is left alone" {
    # Deciding and removing are two operations: replace the scratch
    # between the ownership check and the rm, and the removal acts on
    # a directory the check never examined — the replacement needs no
    # marker to die. The claim has to be atomic with what it claims.
    target="$BATS_TEST_TMPDIR/cleanup-swap"
    beacon="$BATS_TEST_TMPDIR/cleanup-beacon"
    env ARCH_WIRING_SCRATCH="$target" ARCH_WIRING_PAUSE_AFTER_MKDIR=2 \
        ARCH_WIRING_PAUSE_IN_CLEANUP=3 \
        ARCH_WIRING_CLEANUP_BEACON="$beacon" \
        sh "$CHECK" "$RUNNER" "$SUITES" >/dev/null 2>&1 &
    _pid=$!
    _seen=0
    for _ in $(seq 1 50); do
        [ -d "$target" ] && {
            _seen=1
            break
        }
        kill -0 "$_pid" 2>/dev/null || break
        sleep 0.1
    done
    [ "$_seen" -eq 1 ]
    kill -0 "$_pid" 2>/dev/null
    kill -TERM "$_pid" 2>/dev/null
    # The beacon marks the decision made and the window open: swapping
    # before it would exercise the already-covered pre-cleanup swap.
    _decided=0
    for _ in $(seq 1 80); do
        [ -e "$beacon" ] && {
            _decided=1
            break
        }
        kill -0 "$_pid" 2>/dev/null || break
        sleep 0.1
    done
    [ "$_decided" -eq 1 ]
    rm -rf "$target"
    mkdir -p "$target"
    printf 'not yours\n' >"$target/keep-me"
    _status=0
    wait "$_pid" 2>/dev/null || _status=$?
    [ "$_status" -eq 143 ]
    [ -f "$target/keep-me" ]
}

@test "a copied claim token does not surrender a replacement" {
    # The claim file sits in a predictable directory and any same-user
    # process can read it. One that reads the token, replaces the
    # scratch, and copies the token into the replacement makes a
    # content comparison pass — and cleanup then deletes a directory
    # this run never made. Identity has to be something a process
    # replacing the pathname cannot reproduce, and file content is
    # not it.
    target="$BATS_TEST_TMPDIR/claim-copy"
    env ARCH_WIRING_SCRATCH="$target" ARCH_WIRING_PAUSE_AFTER_MKDIR=3 \
        sh "$CHECK" "$RUNNER" "$SUITES" >/dev/null 2>&1 &
    _pid=$!
    _seen=0
    for _ in $(seq 1 50); do
        [ -f "$target/.claim" ] && {
            _seen=1
            break
        }
        kill -0 "$_pid" 2>/dev/null || break
        sleep 0.1
    done
    [ "$_seen" -eq 1 ]
    kill -0 "$_pid" 2>/dev/null
    mv "$target" "$target.stolen"
    mkdir -p "$target"
    cp "$target.stolen/.claim" "$target/.claim"
    printf 'not yours\n' >"$target/keep-me"
    kill -TERM "$_pid" 2>/dev/null
    _status=0
    wait "$_pid" 2>/dev/null || _status=$?
    [ "$_status" -eq 143 ]
    [ -f "$target/keep-me" ] || [ -f "$target.reclaimed.$_pid/keep-me" ]
}

@test "a hard-linked claim does not surrender a replacement" {
    # Holding a descriptor prevents inode reuse but not another hard
    # link to the same inode: a process that links the original claim
    # file into its replacement hands cleanup a path that IS the held
    # file, and an identity carried by the claim file alone then
    # surrenders the replacement. The identity has to be something a
    # replacement owner cannot link — and a directory cannot be
    # hard-linked.
    target="$BATS_TEST_TMPDIR/claim-link"
    env ARCH_WIRING_SCRATCH="$target" ARCH_WIRING_PAUSE_AFTER_MKDIR=3 \
        sh "$CHECK" "$RUNNER" "$SUITES" >/dev/null 2>&1 &
    _pid=$!
    _seen=0
    for _ in $(seq 1 50); do
        [ -f "$target/.claim" ] && {
            _seen=1
            break
        }
        kill -0 "$_pid" 2>/dev/null || break
        sleep 0.1
    done
    [ "$_seen" -eq 1 ]
    kill -0 "$_pid" 2>/dev/null
    mv "$target" "$target.stolen"
    mkdir -p "$target"
    ln "$target.stolen/.claim" "$target/.claim"
    printf 'not yours\n' >"$target/keep-me"
    kill -TERM "$_pid" 2>/dev/null
    _status=0
    wait "$_pid" 2>/dev/null || _status=$?
    [ "$_status" -eq 143 ]
    [ -f "$target/keep-me" ] || [ -f "$target.reclaimed.$_pid/keep-me" ]
}

@test "restoring a stranger's directory never nests it" {
    # Between renaming a foreign occupant aside and putting it back,
    # the path can be taken again. mv onto an existing directory
    # moves the source inside it — cleanup would relocate data this
    # run never owned. When the destination is occupied, the foreign
    # directory stays at the reclaim path instead.
    target="$BATS_TEST_TMPDIR/nest-swap"
    beacon="$BATS_TEST_TMPDIR/nest-beacon"
    env ARCH_WIRING_SCRATCH="$target" ARCH_WIRING_PAUSE_AFTER_MKDIR=2 \
        ARCH_WIRING_PAUSE_IN_CLEANUP=3 \
        ARCH_WIRING_CLEANUP_BEACON="$beacon" \
        sh "$CHECK" "$RUNNER" "$SUITES" >/dev/null 2>&1 &
    _pid=$!
    _seen=0
    for _ in $(seq 1 50); do
        [ -d "$target" ] && {
            _seen=1
            break
        }
        kill -0 "$_pid" 2>/dev/null || break
        sleep 0.1
    done
    [ "$_seen" -eq 1 ]
    kill -0 "$_pid" 2>/dev/null
    # A foreign occupant replaces the scratch before cleanup runs...
    rm -rf "$target"
    mkdir -p "$target"
    printf 'first\n' >"$target/keep-a"
    kill -TERM "$_pid" 2>/dev/null
    _claimed=0
    for _ in $(seq 1 80); do
        [ -e "$beacon" ] && {
            _claimed=1
            break
        }
        kill -0 "$_pid" 2>/dev/null || break
        sleep 0.1
    done
    [ "$_claimed" -eq 1 ]
    # ...and while cleanup holds it aside, the path is taken again.
    mkdir -p "$target"
    printf 'second\n' >"$target/keep-b"
    _status=0
    wait "$_pid" 2>/dev/null || _status=$?
    [ "$_status" -eq 143 ]
    # The second occupant is intact and nothing was nested into it;
    # the first survives wherever cleanup parked it.
    [ -f "$target/keep-b" ]
    [ "$(ls "$target")" = "keep-b" ]
    _first=$(find "$BATS_TEST_TMPDIR" -name keep-a 2>/dev/null | head -1)
    [ -n "$_first" ]
}

@test "the swap case refuses a child that never made a scratch" {
    # Pointed at an uncreatable path the check exits before owning
    # anything; replacing a directory it never made and finding the
    # sentinel intact would describe cleanup nobody ran.
    run ! swap_scenario 3 "$BATS_TEST_TMPDIR/no-such-parent/swap"
}

@test "SIGHUP cleans the scratch like INT and TERM" {
    # A closing terminal or dropped SSH session delivers HUP. With no
    # handler, sh dies without running the EXIT trap, the PID-named
    # scratch stays behind, and a later run that draws the same PID
    # refuses the stale path — an interrupted local run poisoning a
    # future one.
    target="$BATS_TEST_TMPDIR/hup"
    env ARCH_WIRING_SCRATCH="$target" ARCH_WIRING_PAUSE_AFTER_MKDIR=3 \
        sh "$CHECK" "$RUNNER" "$SUITES" >/dev/null 2>&1 &
    pid=$!
    # The scratch has to be seen to exist, and the child has to still
    # be alive to receive the signal — a run that already ended proves
    # nothing about its handlers.
    seen=0
    for _ in $(seq 1 50); do
        [ -d "$target" ] && {
            seen=1
            break
        }
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
    done
    [ "$seen" -eq 1 ]
    kill -0 "$pid" 2>/dev/null
    kill -HUP "$pid" 2>/dev/null
    hup_status=0
    wait "$pid" 2>/dev/null || hup_status=$?
    # 129 either way — signal death and `exit 129` share the code —
    # so the discriminator is the scratch: only a run whose EXIT trap
    # fired has removed it.
    [ "$hup_status" -eq 129 ]
    [ ! -e "$target" ]
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

@test "a workflow that never invokes the runner is not wiring" {
    # The check proves the runner reaches every suite. That is worth
    # nothing if CI never reaches the runner: delete the `Run bats
    # suites` step from bash.yml and the suites stop running while
    # every invariant about the runner stays green.
    gutted="$BATS_TEST_TMPDIR/bash.yml"
    sed '/run-suite\.sh/d' "$REPO_ROOT/.github/workflows/bash.yml" >"$gutted"
    if cmp -s "$gutted" "$REPO_ROOT/.github/workflows/bash.yml"; then
        echo "sabotage removed nothing"
        return 1
    fi
    run ! sh "$CHECK" "$RUNNER" "$SUITES" "$gutted"
}

@test "a run step that merely mentions the runner is not wiring" {
    # `run: echo ./tests/bash/helpers/run-suite.sh` names the runner
    # and never executes it: every ported case disappears from CI
    # while a constraint that only looks for the path on a run line
    # stays green. The runner has to be the command, not a word.
    mentioned="$BATS_TEST_TMPDIR/mentioned.yml"
    sed 's|run: \./tests/bash/helpers/run-suite\.sh|run: echo ./tests/bash/helpers/run-suite.sh|' \
        "$REPO_ROOT/.github/workflows/bash.yml" >"$mentioned"
    if cmp -s "$mentioned" "$REPO_ROOT/.github/workflows/bash.yml"; then
        echo "sabotage changed nothing"
        return 1
    fi
    run ! sh "$CHECK" "$RUNNER" "$SUITES" "$mentioned"
}

@test "a run step that masks the runner's status is not wiring" {
    # `run: ./tests/bash/helpers/run-suite.sh || true` executes every
    # suite and discards the outcome: a failing suite can no longer
    # fail CI. The runner has to be the entire step, aside from
    # whitespace — trailing shell is how a green stops meaning
    # anything.
    masked="$BATS_TEST_TMPDIR/masked.yml"
    sed 's#run: \./tests/bash/helpers/run-suite\.sh#run: ./tests/bash/helpers/run-suite.sh || true#' \
        "$REPO_ROOT/.github/workflows/bash.yml" >"$masked"
    if cmp -s "$masked" "$REPO_ROOT/.github/workflows/bash.yml"; then
        echo "sabotage changed nothing"
        return 1
    fi
    run ! sh "$CHECK" "$RUNNER" "$SUITES" "$masked"
}

@test "the real workflow invokes the runner" {
    run sh "$CHECK" "$RUNNER" "$SUITES" "$REPO_ROOT/.github/workflows/bash.yml"
    [ "$status" -eq 0 ]
}

@test "the signal traps end the run and leave cleanup to EXIT" {
    # `exit` inside a signal trap fires the EXIT trap, so a handler
    # that calls cleanup itself runs it twice — and between the two
    # calls the predictable scratch path can be recreated by someone
    # else, handing the second removal a directory this run never
    # made. A syntactic constraint: the INT and TERM traps may only
    # end the run.
    run ! grep -E "trap 'cleanup.*(INT|TERM)" "$CHECK"
}

@test "a runner that filters every test away is not wiring" {
    # `--filter` selects tests by description; a selector matching
    # nothing runs zero tests while every file still reaches the
    # binary. A stub that reads all arguments as filenames records the
    # real files, ignores the selector, and certifies an empty run.
    copy=$(edited_runner '{ sub(/"\$BATS_BIN" "\$@"/, "\"$BATS_BIN\" --filter no-such-test \"$@\""); print }')
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
