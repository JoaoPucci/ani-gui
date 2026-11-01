#!/usr/bin/env bats
#
# What tools/alongside.sh does with the commands it is given, by
# running it.
#
# rust.yml's picker oracle jobs hand it the model's own tests, the
# pool generator and the picker's build, so the three share the
# runner's cores instead of taking turns. A shell's `&` drops a
# background command's exit status unless something waits for it by
# name, and `set -e` does not reach it, so these cases assert that the
# tool fails whenever any command it ran failed, wherever that command
# stands, and that it fails only once every command has ended. They
# also hold what it does with an empty command (a usage error) and an
# output with no final newline, and what it does when it is itself
# signalled, as the jobs' steps exec it so a cancelled or timed-out
# step signals it: it stops what it started, whatever stage it is at,
# and prints what each command wrote.

load '../helpers/loader'

setup() {
    TOOL="$REPO_ROOT/tools/alongside.sh"
    work=$(mktemp -d "$BATS_TEST_TMPDIR/work-XXXXXX")
    cd "$work" || return
}

# Wait, ten seconds at most, until every named file exists: a command
# touches one once it is running, so a case signals the tool only after
# what it means to interrupt is under way, not after a guessed delay.
until_exists() {
    n=0
    for file in "$@"; do
        until [ -e "$file" ]; do
            n=$((n + 1))
            [ "$n" -le 100 ] || return 1
            sleep 0.1
        done
    done
}

@test "commands that all succeed succeed, their output in the order given" {
    run sh "$TOOL" 'sleep 1; echo first' 'echo second'
    [ "$status" -eq 0 ]
    first=$(printf '%s\n' "$output" | grep -n '^first$' | cut -d: -f1)
    second=$(printf '%s\n' "$output" | grep -n '^second$' | cut -d: -f1)
    [ -n "$first" ] && [ -n "$second" ]
    [ "$first" -lt "$second" ]
}

@test "a command that fails fails the run, wherever it stands" {
    run sh "$TOOL" 'exit 3' 'true' 'true'
    [ "$status" -eq 1 ]
    run sh "$TOOL" 'true' 'exit 3' 'true'
    [ "$status" -eq 1 ]
    run sh "$TOOL" 'true' 'true' 'exit 3'
    [ "$status" -eq 1 ]
}

@test "the failing command is named with its own exit status" {
    run sh "$TOOL" 'true' 'exit 7'
    [ "$status" -eq 1 ]
    printf '%s\n' "$output" | grep -qF '[2] exit 7 (exited 7)'
    run sh "$TOOL" 'exit 7' 'exit 4'
    [ "$status" -eq 1 ]
    printf '%s\n' "$output" | grep -qF '[1] exit 7 (exited 7)'
    printf '%s\n' "$output" | grep -qF '[2] exit 4 (exited 4)'
}

@test "a failure is reported only once every other command has ended" {
    run sh "$TOOL" 'exit 1' 'sleep 1; touch late'
    [ "$status" -eq 1 ]
    [ -e late ]
}

@test "the commands run at the same time" {
    # Each can end only once the other has started: run in turn, the
    # first waits out its ten seconds for a second that never starts.
    run sh "$TOOL" \
        'touch a; n=0; until [ -e b ]; do n=$((n + 1)); [ "$n" -le 100 ] || exit 1; sleep 0.1; done' \
        'touch b; n=0; until [ -e a ]; do n=$((n + 1)); [ "$n" -le 100 ] || exit 1; sleep 0.1; done'
    [ "$status" -eq 0 ]
}

@test "a command killed by a signal fails the run" {
    run sh "$TOOL" 'true' 'kill -9 $$'
    [ "$status" -eq 1 ]
}

@test "a command's standard output and standard error both reach the log" {
    run sh "$TOOL" 'echo to-stdout; echo to-stderr >&2; exit 2'
    [ "$status" -eq 1 ]
    printf '%s\n' "$output" | grep -qx 'to-stdout'
    printf '%s\n' "$output" | grep -qx 'to-stderr'
}

@test "a command is run as written, quotes and all" {
    run sh "$TOOL" 'printf "%s|\n" "two  spaces" '"'"'$HOME'"'"
    [ "$status" -eq 0 ]
    printf '%s\n' "$output" | grep -qxF 'two  spaces|'
    printf '%s\n' "$output" | grep -qxF '$HOME|'
}

@test "no command at all is a usage error, not a success" {
    run sh "$TOOL"
    [ "$status" -eq 2 ]
    printf '%s\n' "$output" | grep -q '^usage: '
}

@test "an empty command is a usage error, not a command that succeeds" {
    run sh "$TOOL" 'true' ''
    [ "$status" -eq 2 ]
    printf '%s\n' "$output" | grep -q '^usage: '
}

@test "output with no final newline still ends before the status line" {
    run sh "$TOOL" 'printf no-newline'
    [ "$status" -eq 0 ]
    printf '%s\n' "$output" | grep -qx 'no-newline'
    printf '%s\n' "$output" | grep -qx '=== \[1\] exited 0'
}

@test "stopped by a signal, it prints what each command wrote so far and fails" {
    # A cancelled or timed-out CI step ends this way: what the commands
    # had written is the only trace of where they stood.
    sh "$TOOL" 'echo started; touch one; sleep 30' 'echo also-started; touch two; sleep 30' >out 2>&1 &
    tool=$!
    until_exists one two
    kill -TERM "$tool"
    status=0
    wait "$tool" || status=$?
    [ "$status" -ne 0 ]
    grep -qx 'started' out
    grep -qx 'also-started' out
}

@test "stopped by a signal, a command that had already ended shows its own status" {
    sh "$TOOL" 'touch one; sleep 30' 'echo done; touch two; exit 4' >out 2>&1 &
    tool=$!
    until_exists one two
    start=$SECONDS
    kill -TERM "$tool"
    status=0
    wait "$tool" || status=$?
    # Stopped, not waited out: the sleep had twenty-nine seconds left.
    [ $((SECONDS - start)) -lt 10 ]
    [ "$status" -eq 143 ]
    grep -qx '=== \[2\] exited 4' out
    grep -qx '=== \[1\] stopped by SIGTERM' out
}

@test "stopped by a signal, it stops what its commands started as well" {
    sh "$TOOL" 'sleep 30 & echo "$!" >grandchild; wait' >out 2>&1 &
    tool=$!
    until_exists grandchild
    start=$SECONDS
    kill -TERM "$tool"
    wait "$tool" || true
    [ $((SECONDS - start)) -lt 10 ]
    [ -s grandchild ]
    sleep 0.5
    run ! kill -0 "$(cat grandchild)"
}

@test "interrupted, it stops what it started and reports, exiting 130" {
    # Run in the foreground: a command started with `&` from a shell
    # without job control starts with SIGINT ignored, and a shell
    # cannot trap a signal it was started ignoring.
    (
        until_exists toolpid one
        kill -INT "$(cat toolpid)"
    ) &
    start=$SECONDS
    run sh -c 'echo "$$" >toolpid; exec sh "$1" "echo started; touch one; sleep 30"' sh "$TOOL"
    [ $((SECONDS - start)) -lt 10 ]
    [ "$status" -eq 130 ]
    printf '%s\n' "$output" | grep -qx 'started'
    printf '%s\n' "$output" | grep -qx '=== \[1\] stopped by SIGINT'
}

@test "signalled while still starting its commands, it stops at once and reports" {
    # The signal comes as soon as the runner has started its first
    # command, which it records in its working directory under TMPDIR;
    # starting six hundred takes it far longer, so the last has not
    # started when the signal comes.
    set --
    i=0
    while [ "$i" -lt 600 ]; do
        i=$((i + 1))
        set -- "$@" 'sleep 30'
    done
    mkdir tmp
    TMPDIR="$work/tmp" sh "$TOOL" "$@" >out 2>&1 &
    tool=$!
    n=0
    until [ -n "$(find tmp -name 1.runner 2>/dev/null)" ]; do
        n=$((n + 1))
        [ "$n" -le 1000 ]
        sleep 0.01
    done
    start=$SECONDS
    kill -TERM "$tool"
    status=0
    wait "$tool" || status=$?
    [ $((SECONDS - start)) -lt 15 ]
    [ "$status" -eq 143 ]
    grep -qx '=== \[600\] not started: SIGTERM came first' out
    # Every command was stopped or never started; none ran to its end.
    run grep -c '^=== \[[0-9]*\] exited' out
    [ "$output" -eq 0 ]
}

@test "a signal before a command leads its own process group still stops it" {
    # A setsid that is slow to start holds the command in the tool's
    # own process group when the signal comes, where signalling the
    # command's group reaches nothing.
    real=$(command -v setsid)
    mkdir shim
    printf '#!/bin/sh\ntouch "%s/in-setsid"\nsleep 3\nexec "%s" "$@"\n' "$work" "$real" >shim/setsid
    chmod +x shim/setsid
    PATH="$work/shim:$PATH" sh "$TOOL" 'echo "$$" >cmdpid; sleep 30' >out 2>&1 &
    tool=$!
    until_exists in-setsid
    start=$SECONDS
    kill -TERM "$tool"
    status=0
    wait "$tool" || status=$?
    [ $((SECONDS - start)) -lt 3 ]
    [ "$status" -eq 143 ]
    grep -qx '=== \[1\] stopped by SIGTERM' out
    sleep 0.5
    if [ -s cmdpid ]; then
        run ! kill -0 "$(cat cmdpid)"
    fi
}
