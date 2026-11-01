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
# each command is waited for by its own process id and its status kept.
# This exits 1 when any command exited non-zero, a command killed by a
# signal included, naming every one that did; 0 when all succeeded; 2
# when given no command or an empty one. It never reports before every
# command has ended; a process a command leaves running in the
# background when it exits is that command's to wait for.
#
# Stopped by SIGINT or SIGTERM, as a cancelled or timed-out CI step is,
# it stops every command still running together with whatever that
# command started, prints what each had written so far, and exits 130
# or 143. Each command runs in a session of its own (setsid, from
# util-linux) so that it leads a process group the stop can signal
# whole; this is a Linux tool.
#
# rust.yml's picker oracle jobs run the model's own tests, the pool
# generator and the picker's build through it, so the three share the
# runner's cores instead of taking turns.
# tests/bash/arch/alongside.bats runs it over each of those outcomes.

set -eu

usage() {
    printf 'usage: sh tools/alongside.sh COMMAND...\n' >&2
    exit 2
}

if [ "$#" -eq 0 ]; then
    usage
fi
for command in "$@"; do
    if [ -z "$command" ]; then
        usage
    fi
done

logs=$(mktemp -d)
trap 'rm -rf "$logs"' EXIT

# Print every command's output, in the order given, between a line
# naming it and a line giving how it ended.
report() {
    n=0
    for command in "$@"; do
        n=$((n + 1))
        printf '=== [%s] %s\n' "$n" "$command"
        if [ -s "$logs/$n.log" ]; then
            cat "$logs/$n.log"
            # A substitution drops a trailing newline, so a last
            # character that survives it is not one.
            last=$(tail -c 1 "$logs/$n.log")
            if [ -n "$last" ]; then
                printf '\n'
            fi
        fi
        if [ -f "$logs/$n.stopped" ]; then
            printf '=== [%s] stopped by SIG%s\n' "$n" "$signal"
        else
            status=$(cat "$logs/$n.status")
            printf '=== [%s] exited %s\n' "$n" "$status"
        fi
    done
}

# stop SIGNAL STATUS COMMAND...: stop every command still running and
# what it started, wait for each, print what each wrote, and exit as a
# shell stopped by that signal does.
stop() {
    trap - INT TERM
    signal=$1
    code=$2
    shift 2
    n=0
    for command in "$@"; do
        n=$((n + 1))
        if [ ! -f "$logs/$n.status" ]; then
            : >"$logs/$n.stopped"
            # A command's runner writes its process id as it starts
            # it; a signal can arrive a moment before.
            tries=0
            while [ ! -s "$logs/$n.pid" ] && [ "$tries" -lt 50 ]; do
                tries=$((tries + 1))
                sleep 0.1
            done
            if [ -s "$logs/$n.pid" ]; then
                pid=$(cat "$logs/$n.pid")
                kill -s TERM -- "-$pid" 2>/dev/null || true
            fi
        fi
    done
    n=0
    for command in "$@"; do
        n=$((n + 1))
        pid=$(cat "$logs/$n.runner")
        wait "$pid" || true
    done
    report "$@"
    exit "$code"
}

trap 'stop INT 130 "$@"' INT
trap 'stop TERM 143 "$@"' TERM

# One runner per command: it starts the command in a session of its
# own, records the command's process id, waits for it, and records its
# exit status as it ends, so the status is kept whatever this shell is
# doing at that moment.
n=0
for command in "$@"; do
    n=$((n + 1))
    (
        trap - INT TERM EXIT
        setsid sh -c "$command" >"$logs/$n.log" 2>&1 &
        printf '%s\n' "$!" >"$logs/$n.pid"
        if wait "$!"; then
            status=0
        else
            status=$?
        fi
        printf '%s\n' "$status" >"$logs/$n.status.tmp"
        mv "$logs/$n.status.tmp" "$logs/$n.status"
    ) &
    printf '%s\n' "$!" >"$logs/$n.runner"
done

n=0
for command in "$@"; do
    n=$((n + 1))
    pid=$(cat "$logs/$n.runner")
    wait "$pid" || true
    if [ ! -f "$logs/$n.status" ]; then
        printf 'alongside: no exit status recorded for [%s] %s\n' "$n" "$command" >&2
        printf '%s\n' 1 >"$logs/$n.status"
    fi
done

trap - INT TERM
report "$@"

failed=0
n=0
for command in "$@"; do
    n=$((n + 1))
    status=$(cat "$logs/$n.status")
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
