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
# Stopped by SIGINT or SIGTERM, as a cancelled or timed-out CI step is
# when the step execs it, it stops every command still running together
# with whatever that command started, prints what each had written so
# far, and exits 130 or 143; a further signal meanwhile is ignored, and
# a command not yet started when the first came is never started. The
# stop sends SIGTERM and waits: a command that ignores it, or one whose
# runner has not recorded its process id within five seconds of the
# signal, is waited for rather than stopped, until whatever signalled
# this kills it outright, as a CI runner does in the end. Each
# command runs in a session of its own (setsid, from util-linux) so
# that it leads a process group the stop can signal whole; this is a
# Linux tool.
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
        status=
        if [ -f "$logs/$n.status" ]; then
            status=$(cat "$logs/$n.status")
        fi
        # One the stop signalled is reported as stopped, unless it ended
        # with a status of its own as the signal came: SIGTERM ends a
        # command with 143.
        if [ -f "$logs/$n.stopped" ] && { [ -z "$status" ] || [ "$status" -eq 143 ]; }; then
            printf '=== [%s] stopped by SIG%s\n' "$n" "$signal"
        elif [ -n "$status" ]; then
            printf '=== [%s] exited %s\n' "$n" "$status"
        else
            printf '=== [%s] not started: SIG%s came first\n' "$n" "$signal"
        fi
    done
}

# stop SIGNAL STATUS COMMAND...: stop every command still running and
# what it started, wait for each, print what each wrote, and exit as a
# shell stopped by that signal does.
stop() {
    # A second signal, as a runner sends SIGTERM after SIGINT, does not
    # cut the report short.
    trap '' INT TERM
    signal=$1
    code=$2
    shift 2
    # A command's runner that has not yet looked for this declines to
    # start its command once it does.
    : >"$logs/stopping"
    n=0
    for command in "$@"; do
        n=$((n + 1))
        # A runner that looked before the flag was set starts its
        # command and records its process id a moment later.
        if [ -f "$logs/$n.alive" ] && [ ! -f "$logs/$n.status" ]; then
            settled "$n"
            if [ -s "$logs/$n.pid" ]; then
                : >"$logs/$n.stopped"
                pid=$(cat "$logs/$n.pid")
                halt "$pid"
            fi
        fi
    done
    # Every runner is a child of this shell, recorded or not: each has
    # declined, or recorded how its command ended, once this returns.
    wait || true
    report "$@"
    exit "$code"
}

# settled N: wait, for five seconds at most, until command N's
# runner has recorded its command's process id or exit status, or has
# declined to start it.
settled() {
    tries=0
    while [ "$tries" -lt 50 ]; do
        if [ -f "$logs/$1.declined" ] || [ -f "$logs/$1.status" ] || [ -s "$logs/$1.pid" ]; then
            return 0
        fi
        tries=$((tries + 1))
        sleep 0.1
    done
}

# halt PID: stop the command whose process is PID, and what it started.
# The command leads a process group of its own once setsid has run;
# until then that group does not exist, and PID is setsid itself, still
# in this shell's group, which is signalled alone so the command never
# starts. setsid may run between the two attempts, so the group is
# tried once more after.
halt() {
    if kill -s TERM -- "-$1" 2>/dev/null; then
        return 0
    fi
    kill -s TERM "$1" 2>/dev/null || true
    kill -s TERM -- "-$1" 2>/dev/null || true
}

trap 'stop INT 130 "$@"' INT
trap 'stop TERM 143 "$@"' TERM

# One runner per command: unless a stop has begun, it starts the
# command in a session of its own, records the command's process id,
# waits for it, and records its exit status as it ends, so the status
# is kept whatever this shell is doing at that moment.
n=0
for command in "$@"; do
    n=$((n + 1))
    (
        trap - INT TERM EXIT
        : >"$logs/$n.alive"
        if [ -f "$logs/stopping" ]; then
            : >"$logs/$n.declined"
            exit 0
        fi
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
