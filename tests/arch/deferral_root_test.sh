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

# This file's own scratch, removed on every exit path including an
# interrupt. It had none: the apostrophe clone used `mktemp -d` and an
# explicit `rm -rf` that only ran when the block completed.
scratch_dir=$(mktemp -d "$REPO_ROOT/tests/arch/.deferral-root.XXXXXX")
cleanup() { [ -n "${scratch_dir:-}" ] && rm -rf "$scratch_dir"; }
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

# The nested clone re-runs this file; it must not recurse. The guard
# carries this file's own name because a generic one is readable from
# any environment: an exported `SKIP_NESTED` — plausible in a shell
# where some other suite uses it — silently removed the apostrophe
# case and left the run green, since the guarded branch prints
# nothing either way.
#
# This is the same defect as the `REPO_ROOT` override two checks up,
# in the same file, reached through a different door. A generic
# variable name is an input from the environment whether or not it was
# meant as one.

# No `eval`. Building a command string means quoting the repository
# path into it, and a checkout under a directory containing an
# apostrophe would then be re-parsed as shell syntax — a test that
# breaks on where it was cloned is the kind of thing this file exists
# to catch elsewhere.
expect_ok() {
    label=$1
    shift
    if "$@" >/dev/null 2>&1; then
        printf '  ok       %s\n' "$label"
    else
        printf '  FAIL     %s\n' "$label"
        failed=1
    fi
}

# Run a script from its own directory, by relative path.
from_arch() {
    (cd "$REPO_ROOT/tests/arch" && sh "./$1")
}

# Run a script with a hostile ambient root set.
with_stray_root() {
    REPO_ROOT=/tmp sh "$REPO_ROOT_ABS/tests/arch/$1"
}

REPO_ROOT_ABS=$REPO_ROOT

printf 'arch/deferral_root_test: invocation and environment\n'

for script in deferral_record agents_contract deferral_record_test; do
    expect_ok "$script.sh resolves the repository when invoked by relative path" \
        from_arch "$script.sh"
done

# A stray REPO_ROOT must not redirect anything. It used to, and the
# script exited 0 against the wrong tree rather than failing.
expect_ok "a stray REPO_ROOT does not redirect the record check" \
    with_stray_root deferral_record.sh
expect_ok "a stray REPO_ROOT does not redirect the contract check" \
    with_stray_root agents_contract.sh

# Every variable these scripts take from the environment must be
# namespaced to this suite. This case exists because the same defect
# was fixed three times here in one review: `REPO_ROOT`, then
# `SKIP_NESTED`, then `DEFERRAL_SIGNAL_PROBE` — each a plausible name
# for something else to export, each silently changing what a check
# did, each found by a reviewer rather than by a run.
#
# Fixing them one at a time is what produced the second and third. A
# name is either namespaced or it is an input from whatever shell the
# suite happens to run in, and that is checkable, so it is checked.
allowed_env='^(ARCH_[A-Z0-9_]+|HOME|PATH|TMPDIR|CI)$'
stray_env=$(grep -hoE '\$\{[A-Z][A-Z0-9_]{2,}:-' "$REPO_ROOT"/tests/arch/*.sh \
    | sed 's/^\${//; s/:-$//' | sort -u \
    | grep -vE "$allowed_env" || true)
if [ -z "$stray_env" ]; then
    printf '  ok       every ambient variable the arch scripts read is namespaced\n'
else
    printf '  FAIL     these are readable from any environment: %s\n' \
        "$(printf '%s' "$stray_env" | tr '\n' ' ')"
    failed=1
fi

# A checkout path containing an apostrophe. This exists because the
# first version of this file built commands as strings and evaluated
# them, so the path was re-parsed as shell syntax and the suite broke
# on where it had been cloned. Verified by hand at the time; asserted
# here so it cannot regress.
# Inside the repository and inside the run's own scratch directory,
# so the cleanup trap removes it on every exit path — including an
# interrupt, which the explicit `rm -rf` at the end of this block
# would miss.
apostrophe_dir="$scratch_dir/o'brien"

# Two properties of the scratch this case creates, checked before it
# is used. It must live inside the repository — the suite has no
# business writing outside the tree it is checking — and it must be
# under the cleanup trap, so an interrupt does not leave a clone in
# the working tree for the next `git status` to report.
case "$apostrophe_dir" in
    "$REPO_ROOT"/*)
        printf '  ok       the apostrophe clone is inside the repository\n' ;;
    *)
        printf '  FAIL     the apostrophe clone is outside the repository: %s\n' "$apostrophe_dir"
        failed=1 ;;
esac
case "$apostrophe_dir" in
    "$scratch_dir"/*)
        printf '  ok       the apostrophe clone is under the cleanup trap\n' ;;
    *)
        printf '  FAIL     the apostrophe clone is not under the cleanup trap\n'
        failed=1 ;;
esac
mkdir -p "$apostrophe_dir"
# Guarded so the nested run does not clone again — but only this
# block. Guarding the whole script made the nested run assert nothing
# and report success for starting up, which the case above now counts
# rather than trusts.
if [ -n "${ARCH_DEFERRAL_NESTED:-}" ]; then
    :
elif git clone -q --depth=1 "$REPO_ROOT" "$apostrophe_dir/repo" 2>/dev/null; then
    # The clone carries committed state only, so an uncommitted change
    # to any of these would go unexercised — the working-tree copies
    # are what this run is meant to be testing.
    cp "$REPO_ROOT/tests/arch/deferral_root_test.sh" \
       "$REPO_ROOT/tests/arch/deferral_record.sh" \
       "$REPO_ROOT/tests/arch/deferral_record_test.sh" \
       "$REPO_ROOT/tests/arch/agents_contract.sh" \
       "$apostrophe_dir/repo/tests/arch/"
    # Count the assertions the nested run makes, rather than trusting
    # its exit status. A guard that skips the whole script would exit
    # zero having checked nothing, and this case would report success
    # for a process that merely started.
    # Both the exit status and the count. The status alone could pass a
    # run that skipped everything; the count alone could pass a run
    # where one case failed while five others succeeded.
    nested_out=$(cd "$apostrophe_dir/repo" \
        && ARCH_DEFERRAL_NESTED=1 sh tests/arch/deferral_root_test.sh 2>&1)
    nested_status=$?
    nested_ok=$(printf '%s\n' "$nested_out" | grep -c '^  ok' || true)
    if [ "$nested_status" -eq 0 ] && [ "${nested_ok:-0}" -ge 5 ]; then
        printf '  ok       the suite asserts (%s cases) from a path containing an apostrophe\n' "$nested_ok"
    else
        printf '  FAIL     nested run: status %s, %s assertions (want 0 and >=5)\n' "$nested_status" "${nested_ok:-0}"
        failed=1
    fi
else
    # Not a skip. This clones a local path to a local path with no
    # network involved, so there is no benign reason for it to fail —
    # a failure means the environment cannot do something this suite
    # depends on, and reporting `ok` for that turns a broken checkout
    # into a green run. The case that made this file necessary was a
    # path the suite could not handle; reporting success when the path
    # was never built is the same defect wearing the opposite sign.
    printf '  FAIL     could not clone for the apostrophe case\n'
    failed=1
fi

[ "$failed" -eq 0 ] || { printf 'arch/deferral_root_test: FAILED\n'; exit 1; }
printf 'arch/deferral_root_test: ok\n'
