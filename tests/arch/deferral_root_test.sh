#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC2312 — command substitutions are read for their text, and a
#       failure arrives as an empty result that the assertion then catches
#
# Scoped to this file rather than widened in SHELLCHECK_OPTS, which
# would also relax the checks guarding the `ani-cli` script itself.
# shellcheck disable=SC2312
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
cleanup() {
    rm -f "$REPO_ROOT/tests/arch/.sabotaged-probe.sh"
    [ -n "${scratch_dir:-}" ] && rm -rf "$scratch_dir"
}
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

# Shell in this repository has to be linted by the shell linter. The
# `ani-cli checks` workflow filters on `**ani-cli`, so a pull request
# touching only `tests/arch/*.sh` never ran shellcheck or shfmt
# against the scripts it added — the checks were green because they
# had not run, which is the same shape as every other finding here.
lint_paths=$(sed -n '/^  pull_request:/,/^jobs:/p' \
    "$REPO_ROOT/.github/workflows/ani-cli.yml" | grep -c 'tests/arch' || true)
if [ "${lint_paths:-0}" -ge 1 ]; then
    printf '  ok       the shell linter runs for changes under tests/arch\n'
else
    printf '  FAIL     tests/arch is not in the shell linter path filter\n'
    failed=1
fi

# Triggering the job is not the same as checking anything. The action
# takes its own exclude list, and a bare `tests` there skips these
# scripts after the workflow has started for them — a job that runs
# and inspects nothing, reported as a pass. The first version of the
# case above asserted only the trigger and would not have caught it.
lint_excludes=$(grep 'sh_checker_exclude' \
    "$REPO_ROOT/.github/workflows/ani-cli.yml" | head -1)
# Read the exclude list as a list, not as a string to pattern-match.
# The first version tested for a bare `tests` and would have passed an
# explicit `tests/arch`, which excludes these scripts just as
# completely — matching one spelling of the problem instead of the
# problem.
excluded_tokens=$(printf '%s' "$lint_excludes" |
    sed 's/.*sh_checker_exclude:[[:space:]]*"//; s/".*//')
arch_excluded=0
lint_subject=tests/arch
for token in $excluded_tokens; do
    case "$lint_subject" in
        "$token" | "$token"/*) arch_excluded=1 ;;
        *) ;;
    esac
done
if [ "$arch_excluded" -eq 0 ]; then
    printf '  ok       the linter does not exclude tests/arch from checking\n'
else
    printf '  FAIL     the linter exclude list covers tests/arch, so it is never checked\n'
    failed=1
fi

# Every variable these scripts read from the environment must be
# namespaced to this suite. A generic name is an input from whatever
# shell the suite happens to run in, whether or not anyone meant it as
# one — which is how `REPO_ROOT` and `SKIP_NESTED` each silently
# changed what a check did, and each was found by a reviewer rather
# than by a run.
#
# Two ways a name is ambient, and both are needed. Reading it with a
# default — `${VAR:-}` or `${VAR:=x}` — says outright that it may be
# absent, which means it may arrive from outside; that holds even when
# the suite also sets it before re-invoking itself, which is exactly
# how a self-invocation guard escapes a plainer test. Reading it
# without a default is ambient only if nothing in the suite ever
# assigns it, since otherwise it is an ordinary local.
#
# Comments are stripped first: this paragraph names the forms it looks
# for, and a check that flags its own prose is measuring the wrong
# thing.
allowed_env='^(ARCH_[A-Z0-9_]+|HOME|PATH|TMPDIR|CI)$'
arch_src=$(sed 's/#.*//' "$REPO_ROOT"/tests/arch/*.sh)
defaulted=$(printf '%s\n' "$arch_src" |
    grep -oE '\$\{[A-Z][A-Z0-9_]{2,}:[-=]' |
    sed 's/^\${//; s/:[-=]$//' | sort -u)
plain=$(printf '%s\n' "$arch_src" |
    grep -oE '\$\{?[A-Z][A-Z0-9_]{2,}\}?' |
    sed 's/^\$//; s/^{//; s/}$//' | sort -u)
assigned=$(printf '%s\n' "$arch_src" |
    grep -oE '^[[:space:]]*[A-Z][A-Z0-9_]{2,}=|^[[:space:]]*for[[:space:]]+[A-Z][A-Z0-9_]{2,}|export[[:space:]]+[A-Z][A-Z0-9_]{2,}' |
    sed 's/^[[:space:]]*//; s/^for[[:space:]]*//; s/^export[[:space:]]*//; s/=$//' |
    sort -u)
if [ -n "$assigned" ]; then
    unowned=$(printf '%s\n' "$plain" | grep -vxF "$assigned" || true)
else
    unowned=$plain
fi
stray_env=$(printf '%s\n%s\n' "$defaulted" "$unowned" |
    grep -v '^$' | sort -u | grep -vE "$allowed_env" || true)
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
        printf '  ok       the apostrophe clone is inside the repository\n'
        ;;
    *)
        printf '  FAIL     the apostrophe clone is outside the repository: %s\n' "$apostrophe_dir"
        failed=1
        ;;
esac
case "$apostrophe_dir" in
    "$scratch_dir"/*)
        printf '  ok       the apostrophe clone is under the cleanup trap\n'
        ;;
    *)
        printf '  FAIL     the apostrophe clone is not under the cleanup trap\n'
        failed=1
        ;;
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
    # The workflow is a subject too, now that a case reads its path
    # filter. Without this the nested run judges the committed copy
    # while the parent judges the tree, and they disagree exactly when
    # the tree is what changed.
    mkdir -p "$apostrophe_dir/repo/.github/workflows"
    cp "$REPO_ROOT/.github/workflows/ani-cli.yml" \
        "$apostrophe_dir/repo/.github/workflows/"
    # Count the assertions the nested run makes, rather than trusting
    # its exit status. A guard that skips the whole script would exit
    # zero having checked nothing, and this case would report success
    # for a process that merely started.
    # Both the exit status and the count. The status alone could pass a
    # run that skipped everything; the count alone could pass a run
    # where one case failed while five others succeeded.
    # `|| nested_status=$?` matters: a bare command substitution that
    # fails aborts the whole script under `set -e`, so a failing nested
    # run used to kill the parent silently instead of being reported.
    # The nested run also skips the sabotage case — it exists to prove
    # the apostrophe path works, not to re-run a sub-suite.
    nested_status=0
    nested_out=$(cd "$apostrophe_dir/repo" &&
        ARCH_DEFERRAL_NESTED=1 ARCH_DEFERRAL_NO_SABOTAGE=1 \
            sh tests/arch/deferral_root_test.sh 2>&1) || nested_status=$?
    nested_ok=$(printf '%s\n' "$nested_out" | grep -c '^  ok' || true)
    if [ "$nested_status" -eq 0 ] && [ "${nested_ok:-0}" -ge 5 ]; then
        printf '  ok       the suite asserts (%s cases) from a path containing an apostrophe\n' "$nested_ok"
    else
        printf '  FAIL     nested run: status %s, %s assertions (want 0 and >=5)\n' "$nested_status" "${nested_ok:-0}"
        failed=1
    fi
else
    # Not a skip. This clones a local path to a local path inside the
    # repository, with no network and no remote, so there is no benign
    # reason for it to fail — a failure means the environment cannot
    # do something this suite depends on, and reporting ok for that
    # turns a broken checkout into a green run.
    printf '  FAIL     could not clone for the apostrophe case\n'
    failed=1
fi

# The library must honour a root the caller resolved, including when
# it is sourced rather than executed. Sourcing is the case that got
# missed: executed, the file derives its own root and that happens to
# be right; sourced by a test that has already pointed at a scratch
# repository, re-deriving silently `cd`s back to this checkout and the
# caller's choice is discarded without a word.
probe_repo=$(mktemp -d "$scratch_dir/sourced-root.XXXXXX")
(
    cd "$probe_repo" && git init -q . &&
        git config user.email t@e && git config user.name t
) >/dev/null 2>&1
sourced_root=$(
    __DEFERRAL_RECORD_LIB__=1 ARCH_REPO_ROOT="$probe_repo" \
        sh -c '. "$1/tests/arch/deferral_record.sh"; pwd' _ "$REPO_ROOT" 2>/dev/null
) || sourced_root=''
case "$sourced_root" in
    "$probe_repo"*)
        printf '  ok       the sourced library stays in the caller resolved root\n'
        ;;
    *)
        printf '  FAIL     sourcing the library left the root at %s\n' \
            "${sourced_root:-unknown}"
        failed=1
        ;;
esac

# An environment that cannot build the clone must fail the run rather
# than report a skip. Nothing about this clone is allowed to fail
# benignly — it is a local path to a local path inside the repository,
# no network and no remote — so a failure means the environment cannot
# do something this suite depends on, and calling that ok states
# something untrue.
#
# Proved by sabotage: a copy of this file with an unsatisfiable
# `--reference` runs the clone for real and cannot complete it. The
# copy skips this case, or it would sabotage a copy of itself forever.
if [ -z "${ARCH_DEFERRAL_NO_SABOTAGE:-}" ]; then
    # Must sit beside the original, not inside the scratch directory:
    # the script derives the repository root from its own path, so a
    # copy one level deeper resolves to `tests/` and fails for a
    # reason that has nothing to do with the clone.
    sabotaged="$REPO_ROOT/tests/arch/.sabotaged-probe.sh"
    sed 's|git clone -q --depth=1|git clone -q --depth=1 --reference /nonexistent-ref|' \
        "$REPO_ROOT/tests/arch/deferral_root_test.sh" >"$sabotaged"
    # Same `set -e` trap as the nested run: this substitution is
    # expected to fail, and a bare one would abort this script instead
    # of letting the case report.
    sabotage_status=0
    sabotage_out=$(ARCH_DEFERRAL_NO_SABOTAGE=1 sh "$sabotaged" 2>&1) ||
        sabotage_status=$?
    if [ "$sabotage_status" -ne 0 ] &&
        printf '%s' "$sabotage_out" | grep -q 'could not clone'; then
        printf '  ok       a clone that cannot be built fails the run\n'
    else
        printf '  FAIL     an unbuildable clone left the run passing (status %s)\n' \
            "$sabotage_status"
        failed=1
    fi
fi

[ "$failed" -eq 0 ] || {
    printf 'arch/deferral_root_test: FAILED\n'
    exit 1
}
printf 'arch/deferral_root_test: ok\n'
