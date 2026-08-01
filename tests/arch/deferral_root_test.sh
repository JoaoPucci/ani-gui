#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC2312 — command substitutions are read for their text, and a
#       failure arrives as an empty result that the assertion then catches
#   SC2310 — helpers are invoked in `if` conditions on purpose, so a
#       failing case reports rather than aborting the run
#
# Scoped to this file rather than widened in SHELLCHECK_OPTS, which
# would also relax the checks guarding the `ani-cli` script itself.
# shellcheck disable=SC2310,SC2312
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
#
# Everything this run creates is named from one prefix, and the prefix
# is a literal fixed before anything exists. That is the whole point,
# and it is not the same as installing the traps early. A name that
# comes back from a creating command cannot be registered in advance
# under any ordering: `mktemp -d` puts the directory on disk when it
# returns and the variable holds the path only once the substitution
# completes, so a signal in that interval leaves something behind that
# the trap has no way to name. Registering a prefix removes the
# dependency instead of racing it — the cleanup refers to nothing a
# creation produced, so there is no interval during which it is
# uninformed.
#
# `$$` keeps concurrent runs apart. The collision-freedom `mktemp` was
# there for is kept by allocating with `set -C`, which fails rather
# than opens when the path is already taken.
tmp_prefix="$REPO_ROOT/tests/arch/.deferral-root.$$"

cleanup() {
    rm -rf "$tmp_prefix" "$tmp_prefix".*
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

# The sabotage probe has to sit beside the original — the script
# derives its root from its own path, so a copy one level deeper
# resolves to `tests/` — which is where the prefix already is.
make_sabotage_probe() {
    # A counter in a variable would not survive. This is called inside
    # command substitutions, and a subshell's increment is lost, so two
    # calls in one line would hand back the same path. The candidate is
    # tried against the filesystem instead, and `set -C` makes the
    # create-if-absent atomic — no window in which two runs agree a
    # name is free.
    _n=0
    while [ "$_n" -lt 1000 ]; do
        _n=$((_n + 1))
        _probe="$tmp_prefix.probe.$_n"
        if (
            set -C
            : >"$_probe"
        ) 2>/dev/null; then
            printf '%s\n' "$_probe"
            return 0
        fi
    done
    return 1
}

# Named above, created here. Plain `mkdir` refuses a path that already
# exists, including a symlink planted at it.
scratch_dir="$tmp_prefix"
mkdir "$scratch_dir"

# Probe mode: the run re-executes itself with this set so the leak
# case watches a real process take a real signal, rather than reading
# trap definitions and hoping they mean what they say. It exits before
# the body, so nothing here re-enters.
if [ -n "${ARCH_SABOTAGE_LEAK_PROBE:-}" ]; then
    printf '%s\n' "$(make_sabotage_probe)" "$(make_sabotage_probe)"
    sleep 5
    exit 0
fi

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

# The runner must not re-parse what it is given as shell syntax. A
# checkout under a directory containing an apostrophe is the case that
# breaks: building a command string interpolates the path into it, and
# the quote then closes a quote the runner opened. This asserts the
# property directly rather than waiting for the nested run to die of
# it further down.
quote_dir="$scratch_dir/o'brien-probe"
mkdir -p "$quote_dir"
printf '#!/bin/sh\nexit 0\n' >"$quote_dir/trivial.sh"
chmod +x "$quote_dir/trivial.sh"

printf 'arch/deferral_root_test: invocation and environment\n'

# Isolated in a subshell: the failure mode is a syntax error at
# evaluation time, which would otherwise kill this script outright and
# report nothing at all.
if (expect_ok probe sh "$quote_dir/trivial.sh") >/dev/null 2>&1; then
    printf '  ok       the runner handles a path containing an apostrophe\n'
else
    printf '  FAIL     the runner breaks on a path containing an apostrophe\n'
    failed=1
fi

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

# The detection, over whatever source it is handed. A function rather
# than a pipeline inline in the live assertion, so a fixture can be run
# through the identical code — asserting only against the real scripts
# means the day it stops detecting anything, it reports ok.
# Shell comments, removed. A `#` opens one only where a word could
# start — at the beginning of a line or after a blank — and only
# outside quotes. `sed 's/#.*//'` honours neither rule, so
# `"#${GENERIC_GUARD-}"` lost its name to a hash that was never a
# comment and the audit reported ok about a guard it had not read.
#
# A line at a time, so a quoted string spanning several resumes
# unquoted on the next. Nothing in these scripts spells one that way,
# and a heredoc body is not shell source to begin with.
#
# SC2016 — the awk program is single-quoted on purpose; `$0` is awk's
# record, not a shell positional.
# shellcheck disable=SC2016
strip_comments() {
    awk '
        BEGIN { sq = sprintf("%c", 39) }
        {
            out = ""; q = ""; prev = ""; i = 1; n = length($0)
            while (i <= n) {
                c = substr($0, i, 1)
                if (q == "") {
                    if (c == "\\") {
                        out = out c substr($0, i + 1, 1); prev = "x"; i += 2; continue
                    }
                    if (c == "#" && (out == "" || prev == " " || prev == "\t")) break
                    if (c == sq || c == "\"") q = c
                } else if (q == "\"" && c == "\\") {
                    out = out c substr($0, i + 1, 1); prev = "x"; i += 2; continue
                } else if (c == q) {
                    q = ""
                }
                out = out c; prev = c; i++
            }
            print out
        }
    ' "$@"
}

ambient_stray_names() {
    _src=$(strip_comments)
    # Every expansion whose result can come from outside: the four
    # POSIX operators, each with and without the colon. The colon only
    # decides whether an empty value counts as unset — it has nothing
    # to do with where the value came from, so requiring it audited
    # half the spellings.
    _defaulted=$(printf '%s\n' "$_src" |
        grep -oE '\$\{[A-Z][A-Z0-9_]*:?[-=?+]' |
        sed 's/^\${//; s/[-:=?+].*$//' | sort -u)
    _plain=$(printf '%s\n' "$_src" |
        grep -oE '\$\{?[A-Z][A-Z0-9_]*\}?' |
        sed 's/^\$//; s/^{//; s/}$//' | sort -u)
    # An export owns a name only when it carries a value. `export NAME`
    # assigns nothing: it marks the name for the environment, and
    # whatever the calling shell already put there survives — so the
    # read stays as ambient as it would be with no export at all.
    _assigned=$(printf '%s\n' "$_src" |
        grep -oE '^[[:space:]]*[A-Z][A-Z0-9_]*=|^[[:space:]]*for[[:space:]]+[A-Z][A-Z0-9_]*|export[[:space:]]+[A-Z][A-Z0-9_]*=' |
        sed 's/^[[:space:]]*//; s/^for[[:space:]]*//; s/^export[[:space:]]*//; s/=$//' |
        sort -u)
    if [ -n "$_assigned" ]; then
        _unowned=$(printf '%s\n' "$_plain" | grep -vxF "$_assigned" || true)
    else
        _unowned=$_plain
    fi
    printf '%s\n%s\n' "$_defaulted" "$_unowned" |
        grep -v '^$' | sort -u | grep -vE "$allowed_env" || true
}

# One uppercase letter is a legal name too, and the pattern still
# required something after it. The shortest thing the audit can see has
# to be the shortest thing the shell accepts.
if ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/single-char-name.sh" |
    grep -qx 'X'; then
    printf '  ok       a single-character name is still ambient\n'
else
    printf '  FAIL     a single-character name escapes the audit\n'
    failed=1
fi

# A two-character name is a legal environment variable, so a guard
# reading one is exactly as ambient as a longer name. `CI` is on the
# allowlist above and is itself two characters — under a pattern that
# cannot match it, that entry never did anything.
if ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/short-name.sh" |
    grep -qx 'NO'; then
    printf '  ok       a two-character name is still ambient\n'
else
    printf '  FAIL     a two-character name escapes the audit entirely\n'
    failed=1
fi

# A colonless POSIX default is exactly as ambient as its colon
# spelling, and the audit has to say so. The fixture pairs one with an
# assignment of the same name, which is what makes the case sharp: the
# assignment takes the name out of the read-but-never-assigned path,
# leaving the default-expansion path as the only thing that can catch
# it. A guard written that way inherits whatever the calling shell
# exported.
#
# The fixture is a file outside tests/arch rather than a string here,
# because this file is itself scanned — test data spelled inline gets
# reported as a real finding.
if ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/colonless-default.sh" |
    grep -qx 'SKIP_NESTED'; then
    printf '  ok       a colonless default expansion is still ambient\n'
else
    printf '  FAIL     a colonless default escapes the audit when the name is also assigned\n'
    failed=1
fi

# A `#` opens a comment only where a word could start, and only
# outside quotes. Stripping from the first one on the line regardless
# swallows the rest of `"#${GENERIC_GUARD-}"` — a legal comparison —
# and the audit then reports ok about a guard whose name it never saw.
if ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/quoted-hash.sh" |
    grep -qx 'GENERIC_GUARD'; then
    printf '  ok       a quoted hash does not hide the rest of the line\n'
else
    printf '  FAIL     a name after a quoted hash escapes the audit\n'
    failed=1
fi

# `export NAME` assigns nothing. It marks the name for the environment
# and whatever the calling shell put there survives, so the read is
# exactly as ambient as it would be with no export at all. Counting
# the bare form as ownership is how a generic guard gets through.
if ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/bare-export.sh" |
    grep -qx 'GENERIC_GUARD'; then
    printf '  ok       a valueless export does not claim ownership\n'
else
    printf '  FAIL     a bare export removes an ambient name from the audit\n'
    failed=1
fi

stray_env=$(strip_comments "$REPO_ROOT"/tests/arch/*.sh | ambient_stray_names)
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

# A stray REPO_ROOT must not redirect anything. It used to, and the
# script exited 0 against the wrong tree rather than failing.
expect_ok "a stray REPO_ROOT does not redirect the record check" \
    with_stray_root deferral_record.sh
expect_ok "a stray REPO_ROOT does not redirect the contract check" \
    with_stray_root agents_contract.sh

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
# A cancelled run must not leave probes behind. Every path the
# allocator hands out has to reach the trap, or a signal between
# allocation and the explicit removal litters the working tree with
# files the next `git status` reports.
#
# Two ways this case could pass without measuring anything, both
# guarded below. If the child dies before printing, there is nothing
# to look for and an unguarded loop reports success having checked no
# paths. And the paths are read a line at a time: a checkout under a
# directory with a space in it splits an unquoted expansion into
# fragments, so the test would stat names that were never files.
leak_out=$(mktemp "$scratch_dir/leak-probe.XXXXXX")
ARCH_SABOTAGE_LEAK_PROBE=1 sh "$REPO_ROOT/tests/arch/deferral_root_test.sh" \
    >"$leak_out" 2>&1 &
leak_pid=$!
i=0
while [ "$i" -lt 50 ] && [ "$(wc -l <"$leak_out")" -lt 2 ]; do
    sleep 0.1
    i=$((i + 1))
done
# Verify the allocations exist while the child still holds them.
# Counting plausible-looking lines is not evidence: two fabricated
# paths, or the same one twice, satisfy any count. A path that is on
# disk now was really allocated, and checking before the signal is the
# only moment that is true.
leak_present=0
leak_seen=""
while IFS= read -r probe; do
    case "$probe" in
        "$REPO_ROOT/tests/arch/.deferral-root."*.probe.*) ;;
        *) continue ;;
    esac
    case "$leak_seen" in
        *"[$probe]"*) continue ;;
        *) leak_seen="${leak_seen}[$probe]" ;;
    esac
    [ -e "$probe" ] && leak_present=$((leak_present + 1))
done <"$leak_out"

kill -TERM "$leak_pid" 2>/dev/null || true
leak_status=0
wait "$leak_pid" 2>/dev/null || leak_status=$?

# Count allocations, not lines. stderr is merged into this file, so
# two diagnostics satisfy a line count while nothing was ever
# allocated — the loop then finds neither string on disk and the case
# reports cleanup succeeded. A line only counts if it is a path the
# allocator could have produced.
leak_count=0
leak_found=0
while IFS= read -r probe; do
    case "$probe" in
        "$REPO_ROOT/tests/arch/.deferral-root."*.probe.*) ;;
        *) continue ;;
    esac
    leak_count=$((leak_count + 1))
    if [ -e "$probe" ]; then
        leak_found=1
        rm -f "$probe"
    fi
done <"$leak_out"

if [ "$leak_present" -ne 2 ]; then
    printf '  FAIL     %s distinct probes existed before the signal, not 2 — nothing was allocated to clean up\n' \
        "$leak_present"
    failed=1
elif [ "$leak_status" -ne 143 ]; then
    printf '  FAIL     the leak probe exited %s, not 143 — it was not the signal that ended it\n' \
        "$leak_status"
    failed=1
elif [ "$leak_found" -eq 0 ]; then
    printf '  ok       a cancelled run leaves no probes behind\n'
else
    printf '  FAIL     a cancelled run left an allocated probe in the working tree\n'
    failed=1
fi

# The sabotage probe must not collide with a file a developer already
# has. A fixed name means `>` truncates whatever sits there and the
# cleanup trap deletes it; had it been a symlink, the redirection
# would have written through to its target. That is the hazard
# `make_untracked_probe` exists for, on the other probe, so the same
# answer applies: allocate a path, never assume one.
#
# Asserted against the allocator directly rather than by running the
# sabotage step, because that step re-executes this file — a case that
# re-enters it has to be reasoned about rather than merely written.
#
# `set -e` would take a missing allocator as a failure of the whole
# script, which is the condition being measured.
alloc_status=0
first_sabotage=$(make_sabotage_probe 2>/dev/null) || alloc_status=$?

if [ "$alloc_status" -ne 0 ] || [ -z "$first_sabotage" ]; then
    printf '  FAIL     no collision-free allocator for the sabotage probe\n'
    failed=1
else
    printf 'do not lose me\n' >"$first_sabotage"
    second_sabotage=$(make_sabotage_probe)
    if [ "$second_sabotage" = "$first_sabotage" ]; then
        printf '  FAIL     the sabotage probe returned the same path twice\n'
        failed=1
    elif [ "$(cat "$first_sabotage" 2>/dev/null)" != 'do not lose me' ]; then
        printf '  FAIL     the sabotage probe clobbered an existing file\n'
        failed=1
    else
        printf '  ok       the sabotage probe never reuses or truncates a path\n'
    fi

    # It also has to sit beside the original: the script derives its
    # root from its own path, so a copy one level deeper resolves to
    # `tests/` and fails for a reason unrelated to the measurement.
    case "$second_sabotage" in
        "$REPO_ROOT/tests/arch/"*)
            printf '  ok       the sabotage probe is allocated beside the original\n'
            ;;
        *)
            printf '  FAIL     the sabotage probe is not beside the original\n'
            failed=1
            ;;
    esac

    # Everything this run creates has to lie under one prefix, and that
    # prefix has to be a literal rather than something a creating
    # command handed back.
    #
    # This is what closes the window a careful ordering of statements
    # cannot. `dir=$(mktemp -d ...)` puts the directory on disk the
    # moment mktemp returns and fills `dir` only when the substitution
    # completes; a signal in between leaves a directory the trap has no
    # way to name, however early the trap was installed. A registered
    # prefix removes the dependency altogether — the cleanup refers to
    # nothing that a creation produced, so there is no interval during
    # which it is uninformed.
    #
    # Asserted over the paths themselves, since the ordering it stands
    # for is not observable after the fact.
    prefix_stem="$REPO_ROOT/tests/arch/.deferral-root.$$"
    prefix_stray=""
    for allocated in "$scratch_dir" "$first_sabotage" "$second_sabotage"; do
        case "$allocated" in
            "$prefix_stem" | "$prefix_stem".*) ;;
            *) prefix_stray="$prefix_stray $allocated" ;;
        esac
    done
    if [ -z "$prefix_stray" ]; then
        printf '  ok       every temporary lies under the one registered prefix\n'
    else
        printf '  FAIL     allocated outside the registered prefix:%s\n' \
            "$prefix_stray"
        failed=1
    fi

    # The other half of registering before creating. A prefix that is
    # already occupied belongs to somebody — a pid comes round again
    # after a kill that skipped cleanup — and the trap, armed before
    # anything was made, would hand it to `rm -rf` as soon as `mkdir`
    # failed. Refusing outright is what keeps the first guarantee from
    # buying the opposite one.
    # Skipped in the child, which carries the variable. A run that
    # refuses exits before reaching here, so the guard matters only
    # while the refusal is absent — which is exactly when this case is
    # meant to fail, and without it that failure is an unbounded fork
    # rather than a report.
    if [ -z "${ARCH_DEFERRAL_TMP_PREFIX:-}" ]; then
        occupied="$scratch_dir/occupied-prefix"
        mkdir -p "$occupied"
        printf 'not yours\n' >"$occupied/keep-me"
        occupied_status=0
        ARCH_DEFERRAL_TMP_PREFIX="$occupied" \
            sh "$REPO_ROOT/tests/arch/deferral_root_test.sh" >/dev/null 2>&1 ||
            occupied_status=$?
        if [ "$occupied_status" -ne 0 ] && [ -f "$occupied/keep-me" ]; then
            printf '  ok       an occupied prefix is refused, not adopted\n'
        else
            printf '  FAIL     an occupied prefix was adopted or emptied (status %s, keep-me %s)\n' \
                "$occupied_status" \
                "$([ -f "$occupied/keep-me" ] && echo present || echo gone)"
            failed=1
        fi

        # A file that merely shares the prefix is the harder half, and
        # it fails quietly. The prefix itself does not exist, so
        # `mkdir` succeeds and the run reports success — then removes
        # somebody else's file on the way out, because the cleanup
        # covers the whole prefix and not only what this run made.
        # Nothing in the output says so.
        shared="$scratch_dir/shared-prefix"
        printf 'not yours\n' >"$shared.probe.999"
        shared_status=0
        ARCH_DEFERRAL_TMP_PREFIX="$shared" \
            sh "$REPO_ROOT/tests/arch/deferral_root_test.sh" >/dev/null 2>&1 ||
            shared_status=$?
        if [ "$shared_status" -ne 0 ] && [ -f "$shared.probe.999" ]; then
            printf '  ok       a file sharing the prefix is refused, not swept\n'
        else
            printf '  FAIL     a file sharing the prefix was swept (status %s, sentinel %s)\n' \
                "$shared_status" \
                "$([ -f "$shared.probe.999" ] && echo present || echo gone)"
            failed=1
        fi
    fi

    rm -f "$first_sabotage" "$second_sabotage"
fi

if [ -z "${ARCH_DEFERRAL_NO_SABOTAGE:-}" ]; then
    # Must sit beside the original, not inside the scratch directory:
    # the script derives the repository root from its own path, so a
    # copy one level deeper resolves to `tests/` and fails for a
    # reason that has nothing to do with the clone.
    sabotage_probe=$(make_sabotage_probe)
    sabotaged="$sabotage_probe"
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
