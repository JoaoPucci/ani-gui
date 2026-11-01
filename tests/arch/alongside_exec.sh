#!/bin/sh

# Architectural invariant: every workflow step that runs
# tools/alongside.sh execs it.
#
# A cancelled or timed-out step is signalled through the shell running
# its script. Run as that shell's child, the runner never sees the
# signal: bash holds a SIGINT until its foreground child ends, and
# dies of a SIGTERM leaving the child running with nowhere to write.
# Exec'd, the runner is the step's process, so the signal reaches it,
# and it stops the commands it started and prints what each had
# written. tests/bash/arch/alongside.bats asserts what it does when
# signalled; this asserts that the workflows let a signal reach it.
#
# What this file asserts is a syntactic constraint, per the AGENTS.md
# rule on how these checks may read source. Leading whitespace aside,
# every line of a workflow that names tools/alongside.sh either is a
# comment or begins `exec sh tools/alongside.sh `, and at least one
# line runs it, so the check cannot pass by having nothing to look at.
#
# Deliberately not covered, and said so: whether the exec'd line is the
# last command of its step (a command after it would never run), and
# whether the step's condition ever holds — telling those apart means
# parsing YAML and the shell. A green run here means only that no
# workflow line runs the tool without exec.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

failed=0
runs=0

if [ ! -f tools/alongside.sh ]; then
    printf 'arch/alongside_exec FAIL: tools/alongside.sh is missing\n' >&2
    failed=1
fi

for workflow in .github/workflows/*.yml; do
    lines=$(grep -n 'tools/alongside\.sh' "$workflow" || true)
    if [ -z "$lines" ]; then
        continue
    fi
    while IFS= read -r line; do
        number=${line%%:*}
        text=${line#*:}
        # Leading whitespace aside.
        trimmed=$(printf '%s\n' "$text" | sed 's/^[[:space:]]*//')
        case "$trimmed" in
            '#'*) ;;
            'exec sh tools/alongside.sh '*) runs=$((runs + 1)) ;;
            *)
                printf 'arch/alongside_exec FAIL: %s:%s runs tools/alongside.sh without exec: %s\n' \
                    "$workflow" "$number" "$trimmed" >&2
                failed=1
                ;;
        esac
    done <<EOF
$lines
EOF
done

if [ "$runs" -eq 0 ]; then
    printf 'arch/alongside_exec FAIL: no workflow line execs tools/alongside.sh\n' >&2
    failed=1
fi

exit "$failed"
