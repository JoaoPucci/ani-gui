#!/bin/sh
# The divergence ceiling has to admit the patch set we mean to carry.
#
# `bash_portability.sh` counts how far the vendored `ani-cli` differs
# from upstream and fails past a ceiling, so an edit cannot creep in
# unrecorded. That only works if an unmodified checkout passes: a
# ceiling everything fails against reports the same thing whether or
# not a new patch landed, which is no report at all.
#
# So this asserts the check is green against the script as committed,
# with the five patches AGENTS.md §3 documents and nothing else.

set -eu

REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$REPO_ROOT"

if ! git remote get-url upstream >/dev/null 2>&1; then
    printf 'arch/bash_portability_test: no upstream remote — skipping\n'
    exit 0
fi

if sh "$SCRIPT_DIR/bash_portability.sh" >/dev/null 2>&1; then
    printf 'arch/bash_portability_test: ok (the documented patch set is under the ceiling)\n'
else
    printf 'arch/bash_portability_test: FAILED — the check rejects an unmodified checkout, so it cannot distinguish a new patch from the standing failure\n'
    exit 1
fi
