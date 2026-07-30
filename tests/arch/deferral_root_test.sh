#!/bin/sh
# The checks must resolve this repository regardless of how they are
# invoked, and must not be redirectable by a stray environment.
#
# Both properties were broken in turn by the same seam. The library
# re-derived its own root from `$0`, which is meaningless once a caller
# has changed directory, so an invocation by relative path from
# `tests/arch` walked above the repository. Honouring an inherited
# `REPO_ROOT` fixed that and introduced the other half: `REPO_ROOT` is
# a common name, so any environment defining one redirected the check
# — silently, exiting 0 against the wrong tree.
#
# Only `ARCH_REPO_ROOT` overrides now, and callers hand the resolved
# root over under that name. These are the cases that pins it.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

failed=0

check() {
    if [ "$2" = pass ]; then
        if eval "$1" >/dev/null 2>&1; then
            printf '  ok       %s\n' "$3"
        else
            printf '  FAIL     %s\n' "$3"; failed=1
        fi
    else
        if eval "$1" >/dev/null 2>&1; then
            printf '  FAIL     %s\n' "$3"; failed=1
        else
            printf '  ok       %s\n' "$3"
        fi
    fi
}

printf 'arch/deferral_root_test: invocation and environment\n'

for s in deferral_record agents_contract deferral_record_test; do
    check "(cd '$REPO_ROOT/tests/arch' && sh ./$s.sh)" pass \
        "$s.sh resolves the repository when invoked by relative path"
done

# A stray REPO_ROOT must not redirect anything. It used to, and the
# script exited 0 against the wrong tree rather than failing.
check "REPO_ROOT=/tmp sh '$REPO_ROOT/tests/arch/deferral_record.sh'" pass \
    "a stray REPO_ROOT does not redirect the record check"
check "REPO_ROOT=/tmp sh '$REPO_ROOT/tests/arch/agents_contract.sh'" pass \
    "a stray REPO_ROOT does not redirect the contract check"

[ "$failed" -eq 0 ] || { printf 'arch/deferral_root_test: FAILED\n'; exit 1; }
printf 'arch/deferral_root_test: ok\n'
