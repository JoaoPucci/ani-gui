#!/bin/sh
# Architectural invariant: every e2e spec takes `test` and `expect`
# from the content security policy guard, electron/e2e/policy-guard.ts.
#
# The renderer runs under a policy, and a violation is silent to the
# user: a poster that never loads, a stream that never attaches. The
# guard fails a test on any violation it did not declare, but only a
# test declared through the guard's own `test`: a spec that takes
# `test` from '@playwright/test' still launches guarded (the shared
# launch helper listens on every app it starts) and is never failed
# for what the guard heard. A spec added that way passes green with
# its violations unread.
#
# A syntactic constraint, not interpretation: each *.spec.ts under
# electron/e2e/, at any depth (the flags specs live in
# electron/e2e/flags/), carries an import from the guard by a relative
# path ('./policy-guard', '../policy-guard'). Whether that import names
# `test` is not read; a spec that imported only a type from the guard,
# and `test` from Playwright, would pass this check. None does; this
# check would not notice one. Specs outside electron/e2e/ are not
# scanned.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

GUARD='electron/e2e/policy-guard.ts'
failed=0

if [ ! -f "$GUARD" ]; then
    printf 'arch/e2e_policy_guard FAIL: the policy guard %s does not exist\n' "$GUARD" >&2
    exit 1
fi

found=0
specs=$(find electron/e2e -name node_modules -prune -o -type f -name '*.spec.ts' -print | LC_ALL=C sort)
for spec in $specs; do
    found=1
    if ! grep -qE "from ['\"](\./|(\.\./)+)policy-guard['\"]" "$spec"; then
        printf 'arch/e2e_policy_guard FAIL: %s does not import from the policy guard;\n' "$spec" >&2
        printf 'take test and expect from %s, so a policy violation fails the test\n' "$GUARD" >&2
        failed=1
    fi
done

if [ "$found" = 0 ]; then
    printf 'arch/e2e_policy_guard FAIL: no spec under electron/e2e/ to check\n' >&2
    failed=1
fi

exit "$failed"
