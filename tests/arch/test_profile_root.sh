#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC2312 — command substitutions are read for their text, and a
#       failure arrives as an empty result that the assertion catches
#
# shellcheck disable=SC2312
# Architectural invariant: only the backend's test harness sets the
# test-only profile root.
#
# `ANI_GUI_TEST_PROFILE_ROOT` roots every profile path the backend
# resolves — config, cache, state, logs, the default download folder —
# under one folder, ahead of the XDG variables, `HOME` and the
# platform's folder APIs (backend/src/config/paths.rs). It exists so
# the integration tests that spawn the real binary keep it off the
# machine's own profile on Windows and macOS. A launch that set it
# would put a user's library somewhere they never chose, so the app
# itself must never set it: not Electron, not a packaging script, not
# a workflow, not the backend outside the module that reads it.
#
# A syntactic constraint, per the AGENTS.md rule on how these checks
# may read source: the variable's name, and the name of the constant
# the backend spells it with, appear in tracked files only at the
# paths listed below.
#
# Deliberately not covered, and said so: a name assembled at run time
# from pieces passes here, and so does an environment that already
# carries the variable — Electron hands the backend its own
# environment, so a value set by whoever launched the app reaches it.
# A green run means only that nothing in this repository spells the
# name outside its allowed places.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# A grep that cannot run finds nothing, which would read as a pass.
git rev-parse --is-inside-work-tree >/dev/null

# Allowed: the reader, the backend's tests, the docs that describe it
# as test-only, and this file, which has to name what it guards.
offending=$(
    git -c core.quotePath=false grep -l -F \
        -e ANI_GUI_TEST_PROFILE_ROOT -e TEST_PROFILE_ROOT_ENV -- . |
        while IFS= read -r file; do
            case "$file" in
                backend/src/config/paths.rs) ;;
                backend/src/config/paths_test.rs) ;;
                backend/tests/*) ;;
                docs/development.md) ;;
                docs/testing.md) ;;
                tests/arch/test_profile_root.sh) ;;
                *) printf '%s\n' "$file" ;;
            esac
        done
)

if [ -n "$offending" ]; then
    printf 'arch/test_profile_root FAIL: only the backend test harness may name the\n' >&2
    printf 'test-only profile root (backend/src/config/paths.rs). Named in:\n%s\n' "$offending" >&2
    exit 1
fi
exit 0
