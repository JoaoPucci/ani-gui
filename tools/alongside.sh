#!/bin/sh
# Run every argument as a shell command, all at the same time, and fail
# when any of them failed. From the repository root:
#
#     sh tools/alongside.sh 'first command' 'second command' ...
#
# Each command runs under `sh -c` exactly as written, with standard
# output and standard error captured together. Once every command has
# ended, the output of each is printed in the order the commands were
# given, between a line naming the command and a line giving its exit
# status, so the log reads as if they had run in turn.
#
# A command started with `&` keeps its exit status only until something
# waits for that process by name, and `set -e` does not reach it, so
# this waits for each process by its own id and keeps what each
# returned. It exits 1 when any command exited non-zero, a command
# killed by a signal included, naming every one that did; 0 when all
# succeeded; 2 when given no command. It never reports before every
# command has ended.
#
# rust.yml's picker oracle jobs run the model's own tests, the pool
# generator and the picker's build through it, so the three share the
# runner's cores instead of taking turns.
# tests/bash/arch/alongside.bats runs it over each of those outcomes.

set -eu

if [ "$#" -eq 0 ]; then
    printf 'usage: sh tools/alongside.sh COMMAND...\n' >&2
    exit 2
fi

logs=$(mktemp -d)
trap 'rm -rf "$logs"' EXIT

n=0
for command in "$@"; do
    n=$((n + 1))
    sh -c "$command" >"$logs/$n.log" 2>&1 &
    printf '%s\n' "$!" >"$logs/$n.pid"
done

n=0
for command in "$@"; do
    n=$((n + 1))
    pid=$(cat "$logs/$n.pid")
    if wait "$pid"; then
        status=0
    else
        status=$?
    fi
    printf '%s\n' "$status" >"$logs/$n.status"
done

failed=0
n=0
for command in "$@"; do
    n=$((n + 1))
    status=$(cat "$logs/$n.status")
    printf '=== [%s] %s\n' "$n" "$command"
    cat "$logs/$n.log"
    printf '=== [%s] exited %s\n' "$n" "$status"
    if [ "$status" -ne 0 ]; then
        failed=1
    fi
done

if [ "$failed" -ne 0 ]; then
    printf 'alongside: failed:\n' >&2
    n=0
    for command in "$@"; do
        n=$((n + 1))
        status=$(cat "$logs/$n.status")
        if [ "$status" -ne 0 ]; then
            printf '  [%s] %s (exited %s)\n' "$n" "$command" "$status" >&2
        fi
    done
    exit 1
fi
