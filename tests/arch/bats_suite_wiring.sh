#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC2312 — command substitutions are read for their text, and a
#       failure arrives as an empty result that the assertion catches
#
# shellcheck disable=SC2312
# Architectural invariant: every bats suite is actually invoked.
#
# `tests/bash/helpers/run-suite.sh` walks a hard-coded list of suite
# names. A directory full of `.bats` files that is not on that list is
# never run, and nothing says so — the job passes having executed the
# suites it does know about.
#
# This cannot be checked from inside the bats suites themselves. A test
# that has not run cannot report its own absence, so a case under
# `tests/bash/arch/` asserting the arch suite is wired is satisfied
# only when the thing it doubts is already true. The check has to live
# where it runs unconditionally, which is here: `run-all.sh` executes
# every `tests/arch/*.sh` on every pull request.

set -eu

REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$REPO_ROOT"

RUNNER="${1:-$REPO_ROOT/tests/bash/helpers/run-suite.sh}"
SUITES_DIR="${2:-$REPO_ROOT/tests/bash}"

if [ ! -f "$RUNNER" ]; then
    printf 'arch/bats_suite_wiring: %s is missing\n' "$RUNNER" >&2
    exit 1
fi

# The names the runner walks. Read from the loop itself rather than
# from the file at large, so a suite named only in a comment does not
# count as wired.
wired=$(sed -n 's/^[[:space:]]*for suite in \(.*\); do$/\1/p' "$RUNNER")

if [ -z "$wired" ]; then
    printf 'arch/bats_suite_wiring: no suite loop found in %s — the runner shape changed and this check can no longer read it\n' \
        "$RUNNER" >&2
    exit 1
fi

failed=0

# Every directory holding bats files has to be on that list. The
# vendored bats carries its own fixtures and is not ours to run.
for dir in "$SUITES_DIR"/*/; do
    [ -d "$dir" ] || continue
    name=$(basename "$dir")
    case "$name" in
        .bats-vendor | helpers) continue ;;
        *) ;;
    esac
    if [ -z "$(find "$dir" -type f -name '*.bats' 2>/dev/null)" ]; then
        continue
    fi
    found=0
    for suite in $wired; do
        [ "$suite" = "$name" ] && found=1
    done
    if [ "$found" -eq 0 ]; then
        printf 'arch/bats_suite_wiring FAIL: %s holds bats files and is not in the runner loop — those cases never run, and nothing reports it\n' \
            "tests/bash/$name" >&2
        failed=1
    fi
done

[ "$failed" -eq 0 ] || exit 1
printf 'arch/bats_suite_wiring: ok (every bats suite is in the runner loop)\n'
