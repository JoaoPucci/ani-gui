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
# stands, and that it fails only once every command has ended.

load '../helpers/loader'

setup() {
    TOOL="$REPO_ROOT/tools/alongside.sh"
    work=$(mktemp -d "$BATS_TEST_TMPDIR/work-XXXXXX")
    cd "$work" || return
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
    sh "$TOOL" 'echo started; sleep 30' 'echo also-started; sleep 30' >out 2>&1 &
    tool=$!
    sleep 1
    kill -TERM "$tool"
    status=0
    wait "$tool" || status=$?
    [ "$status" -ne 0 ]
    grep -qx 'started' out
    grep -qx 'also-started' out
}
