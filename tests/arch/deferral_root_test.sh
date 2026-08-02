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
#
# `ARCH_DEFERRAL_TMP_PREFIX` exists so the cases below can hand this a
# prefix they control.
tmp_prefix="${ARCH_DEFERRAL_TMP_PREFIX:-$REPO_ROOT/tests/arch/.deferral-root.$$}"

cleanup() {
    rm -rf "$tmp_prefix" "$tmp_prefix".*
}

# Registering the prefix before creating anything buys the guarantee
# above and costs the opposite one: the cleanup sweeps the whole
# prefix, so anything already sitting there goes with it. A pid comes
# round again after a kill that skipped cleanup, so that is reachable
# rather than theoretical, and it has two shapes.
#
# The prefix itself occupied fails loudly — `mkdir` refuses and the
# run dies with the trap armed. A sibling merely sharing the name
# fails quietly, which is worse: the directory does not exist, `mkdir`
# succeeds, the run reports success and removes somebody else's file
# on the way out.
#
# Both are refused here, before anything is armed. Tracking only what
# this run created is the other way to answer it, and it is the way
# that was already tried — a manifest cannot be written until the path
# it names exists, which is the interval this whole arrangement is
# built to remove.
# `test -e` follows the link and answers about the target, so a
# dangling symlink reads as free — survives the check, fails the
# `mkdir`, and is then removed by the cleanup this refusal exists to
# prevent. `-L` asks about the link itself, which is the thing that is
# actually in the way.
prefix_taken=""
if [ -e "$tmp_prefix" ] || [ -L "$tmp_prefix" ]; then
    prefix_taken="$tmp_prefix"
else
    for existing in "$tmp_prefix".*; do
        if [ -e "$existing" ] || [ -L "$existing" ]; then
            prefix_taken="$existing"
            break
        fi
    done
fi
if [ -n "$prefix_taken" ]; then
    printf 'arch/deferral_root_test: %s already exists — refusing, because this run cleans by prefix and would remove a path it did not create\n' \
        "$prefix_taken" >&2
    exit 1
fi

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
    # Basenames only. An absolute path carries the checkout prefix,
    # and a newline anywhere in that prefix splits one record into
    # several — the reader then matches nothing. The basename is
    # `.deferral-root.<pid>.probe.<n>`, an alphabet this run controls.
    _p1=$(make_sabotage_probe)
    _p2=$(make_sabotage_probe)
    printf '%s\n' "${_p1##*/}" "${_p2##*/}"
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

# Shell in this repository has to be linted by the shell linter, and
# the green has to be attributable. `Shellcheck + Shfmt` is reported
# by three workflows — the vendored `ani-cli checks`, its instant
# inverse stub, and the unconditional mirror whose exclude list
# covers all of `tests` — and branch protection is satisfied by the
# first success under a required name. On a pull request touching
# only the arch checks, a stub's echo can land before the real lint
# finishes, so a shellcheck failure in these load-bearing scripts
# could still merge. no-awk-required.yml documents the same race and
# resolves it the same way: a name nothing else answers for.
arch_lint_workflow="$REPO_ROOT/.github/workflows/arch-lint.yml"
arch_lint_name='Arch Shellcheck + Shfmt'
# The same name with its one regex-special character escaped, for the
# count below.
arch_lint_name_re='Arch Shellcheck \+ Shfmt'

# How many lines on stdin declare the arch lint's check name, as a
# function so a fixture spelling runs through the same count the live
# scan runs. YAML's bare, double- and single-quoted spellings all
# make the same declaration, so all three count — and each ends at
# its closing delimiter, because a longer name is a different check.
# A block-scalar `name:` counts too: the name it resolves to sits on
# the next physical line where this count cannot read it, so any such
# declaration is treated as a potential producer — refusal by
# over-count, failing closed. The same refusal covers the quoted
# spellings this count cannot read: a double-quoted value containing a
# backslash resolves through escapes it does not interpret, and a
# quote left open on the line continues the scalar past where it
# reads. Single-quoted scalars have no escapes, so only their
# unterminated form is unreadable.
count_arch_lint_names() {
    grep -cE "name:[[:space:]]*(\"$arch_lint_name_re\"|'$arch_lint_name_re'|${arch_lint_name_re}[[:space:]]*([]#,}].*)?\$|[>|]|\"[^\"]*\\\\[^\"]*\"|\"[^\"]*\$|'[^']*\$)" || true
}

# Every workflow file in a directory, as a function so a fixture
# directory runs through the same enumeration the live scan runs.
# GitHub loads both extensions, so both are read.
scan_workflows() {
    cat "$1"/*.yml "$1"/*.yaml 2>/dev/null || true
}

producer_lines=$(scan_workflows "$REPO_ROOT/.github/workflows" |
    count_arch_lint_names)
if [ "${producer_lines:-0}" -eq 1 ]; then
    printf '  ok       exactly one workflow reports the arch lint check name\n'
else
    printf '  FAIL     %s workflows report the arch lint check name, so another job can answer for the lint\n' \
        "${producer_lines:-0}"
    failed=1
fi

# YAML accepts the double- and single-quoted spellings as the same
# declaration the bare form makes. A count reading only the bare form
# stays at one while a quoted second producer answers for the
# required name — the exact ambiguity the case above exists to
# reject.
quoted_probe=$(printf '%s\n' "    name: \"$arch_lint_name\"" |
    count_arch_lint_names)
single_probe=$(printf '%s\n' "    name: '$arch_lint_name'" |
    count_arch_lint_names)
if [ "${quoted_probe:-0}" -eq 1 ] && [ "${single_probe:-0}" -eq 1 ]; then
    printf '  ok       a quoted spelling of the check name counts as a producer\n'
else
    printf '  FAIL     quoted spellings count %s and %s producers, not 1 and 1\n' \
        "${quoted_probe:-0}" "${single_probe:-0}"
    failed=1
fi

# A block scalar declares the same name on the next physical line:
# `name: >-` followed by the indented name resolves to the job name
# GitHub sees, while a single-line count sees nothing. Missed, a
# second producer hides behind the fold; counted as a producer, any
# block-scalar job name fails the uniqueness case loudly — refusal by
# over-count, the failing-closed direction.
block_dir="$scratch_dir/block-name"
mkdir "$block_dir"
printf 'jobs:\n  a:\n    name: >-\n      %s\n' "$arch_lint_name" >"$block_dir/a.yml"
block_count=$(scan_workflows "$block_dir" | count_arch_lint_names)
if [ "${block_count:-0}" -ge 1 ]; then
    printf '  ok       a block-scalar name declaration is not silently missed\n'
else
    printf '  FAIL     a block-scalar producer counts %s — invisible to the scan\n' \
        "${block_count:-0}"
    failed=1
fi
rm -rf "$block_dir"

# GitHub loads workflows with either extension. A scan reading only
# `*.yml` never sees a `duplicate.yaml` declaring the name, and the
# uniqueness case certifies a name two jobs answer for while the file
# that breaks it sits beside the ones it read.
yaml_dir="$scratch_dir/yaml-scan"
mkdir "$yaml_dir"
printf 'jobs:\n  a:\n    name: %s\n' "$arch_lint_name" >"$yaml_dir/a.yml"
printf 'jobs:\n  b:\n    name: %s\n' "$arch_lint_name" >"$yaml_dir/b.yaml"
yaml_count=$(scan_workflows "$yaml_dir" | count_arch_lint_names)
if [ "${yaml_count:-0}" -eq 2 ]; then
    printf '  ok       a .yaml workflow enters the producer scan\n'
else
    printf '  FAIL     a fixture directory with a .yml and a .yaml producer counts %s, not 2\n' \
        "${yaml_count:-0}"
    failed=1
fi
rm -rf "$yaml_dir"

# A longer name is a different check: `Arch Shellcheck + Shfmt
# (legacy)` cannot answer for the required name, so counting it makes
# the uniqueness case fail with two producers while only one job
# holds the name that matters. The count has to stop at the name's
# closing delimiter — the quote it opened with, or the end of the
# bare scalar.
suffix_probe=$(printf '%s\n' "    name: $arch_lint_name (legacy)" |
    count_arch_lint_names)
suffix_quoted_probe=$(printf '%s\n' "    name: \"$arch_lint_name (legacy)\"" |
    count_arch_lint_names)
if [ "${suffix_probe:-0}" -eq 0 ] && [ "${suffix_quoted_probe:-0}" -eq 0 ]; then
    printf '  ok       a longer name does not count as a producer\n'
else
    printf '  FAIL     longer names count %s and %s producers, not 0 and 0\n' \
        "${suffix_probe:-0}" "${suffix_quoted_probe:-0}"
    failed=1
fi

# A bare scalar ends at the line's end or at an inline comment —
# `name: Arch Shellcheck + Shfmt # required` resolves to exactly the
# required name, and an end-anchored-only pattern misses it.
comment_probe=$(printf '%s\n' "    name: $arch_lint_name # required job" |
    count_arch_lint_names)
if [ "${comment_probe:-0}" -eq 1 ]; then
    printf '  ok       a commented bare name counts as a producer\n'
else
    printf '  FAIL     a commented bare name counts %s producers, not 1\n' \
        "${comment_probe:-0}"
    failed=1
fi

# Flow style puts the whole job on one line: in
# `jobs: {stub: {name: Arch Shellcheck + Shfmt}}` the bare scalar
# ends at the closing brace, not at the line's end. The flow
# terminators `}`, `,` and `]` complete the bare form's delimiter set
# alongside the comment. The count does not track flow context, so a
# block-context line whose scalar happens to continue with one of
# these characters spells a longer name yet still counts — an
# over-count that fails the uniqueness case loudly, the failing-closed
# direction.
flow_probe=$(printf '%s\n' "    jobs: {stub: {name: $arch_lint_name}}" |
    count_arch_lint_names)
if [ "${flow_probe:-0}" -eq 1 ]; then
    printf '  ok       a flow-style name counts as a producer\n'
else
    printf '  FAIL     a flow-style name counts %s producers, not 1\n' \
        "${flow_probe:-0}"
    failed=1
fi

# A double-quoted scalar can spell the name through escapes —
# `"Arch Shellcheck \u002b Shfmt"` resolves to exactly the required
# name — and a quote left open on the line continues the scalar where
# a line count cannot follow. Neither spelling is readable here, so
# both count as potential producers: refusal by over-count, the same
# failing-closed arm the block scalar takes. Single-quoted scalars
# have no escapes — a backslash there is a literal, a different name —
# so only the unterminated form of that spelling is unreadable.
escaped_probe=$(printf '%s\n' '    name: "Arch Shellcheck \u002b Shfmt"' |
    count_arch_lint_names)
open_dq_probe=$(printf '%s\n' "    name: \"$arch_lint_name" |
    count_arch_lint_names)
open_sq_probe=$(printf '%s\n' "    name: '$arch_lint_name" |
    count_arch_lint_names)
if [ "${escaped_probe:-0}" -ge 1 ] && [ "${open_dq_probe:-0}" -ge 1 ] &&
    [ "${open_sq_probe:-0}" -ge 1 ]; then
    printf '  ok       an unreadable quoted spelling is not silently missed\n'
else
    printf '  FAIL     unreadable quoted spellings count %s, %s and %s — invisible to the scan\n' \
        "${escaped_probe:-0}" "${open_dq_probe:-0}" "${open_sq_probe:-0}"
    failed=1
fi

# A path filter would reopen both gaps at once: a pull request the
# filter misses never lints these scripts, and the mirror pair that
# papers over the zero-diff case is a second producer of the name
# again. No filter, no gap, no race.
if [ -f "$arch_lint_workflow" ] &&
    ! sed -n '/^on:/,/^[a-z]/p' "$arch_lint_workflow" | grep -q 'paths'; then
    printf '  ok       the arch lint workflow fires unconditionally\n'
else
    printf '  FAIL     the arch lint workflow is missing or path-gated\n'
    failed=1
fi

# Starting the job is not the same as checking anything. The action
# takes its own exclude list, and a bare `tests` there skips these
# scripts after the workflow has started for them — a job that runs
# and inspects nothing, reported as a pass.
#
# Read the exclude list as a list, not as a string to pattern-match.
# An earlier version tested for a bare `tests` and would have passed
# an explicit `tests/arch`, which excludes these scripts just as
# completely — matching one spelling of the problem instead of the
# problem.
#
# A function over stdin, so a fixture spelling runs through the same
# code as the live line.
#
# Only the paired-quote forms extract: a value whose quote closes on
# the line it opened on, double or single. Everything else — an
# unterminated quote, a block scalar, and the bare plain form, which
# can continue onto the next line with no first-line signal at all —
# passes through untouched, and the surviving key text lands in the
# refusal. The readable set is exactly the spellings whose first line
# provably carries the whole value.
parse_exclusions() {
    sed "s/.*sh_checker_exclude:[[:space:]]*\"\([^\"]*\)\".*/\1/; t
s/.*sh_checker_exclude:[[:space:]]*'\([^']*\)'.*/\1/; t"
}

# The refusal decision over an extracted value, as a function for the
# same reason: a fixture spelling has to ask exactly the question the
# live line asks. Three ways an extraction fails, all refused: the key
# text survives into the value; the value opens with YAML syntax — a
# block scalar (`>`, `|`), flow collection (`[`, `{`), anchor or
# alias (`&`, `*`) — meaning the list lives somewhere this
# line-oriented read never looked; or the value carries an escape the
# extraction does not resolve, so the tokens as read are not the
# tokens the action receives. An empty value is not refused: a key
# with nothing after it excludes nothing, and that is a correct read.
exclusions_unreadable() {
    case "$1" in
        *sh_checker_exclude*) return 0 ;;
        '>'* | '|'* | '['* | '{'* | '&'* | '*'*) return 0 ;;
        *\\*) return 0 ;;
        *) return 1 ;;
    esac
}

# The extraction has to survive the quote spellings the action accepts.
# Stripping only double quotes leaves a single-quoted value carrying
# its quote characters, the tokens then match nothing, and the check
# reports the scripts linted while the action still excludes them.
lint_probe=$(printf "%s\n" "      sh_checker_exclude: 'tests/probe other'" |
    parse_exclusions)
if [ "$lint_probe" = 'tests/probe other' ]; then
    printf '  ok       the exclusion extraction survives a single-quoted value\n'
else
    printf '  FAIL     a single-quoted exclusion list is read as %s\n' "${lint_probe:-nothing}"
    failed=1
fi

# YAML also spells the value as a block scalar: `sh_checker_exclude: >-`
# with the list on the next line. Line-oriented extraction of that line
# yields the fold marker while the value goes unread — no key text
# survives, so nothing trips the refusal, and an exclude list that does
# cover tests/arch scans as not covering it. The boundary stands: this
# check reads the inline spellings only. But a spelling past the
# boundary has to arrive as a refusal, not as a pass.
block_probe=$(printf '%s\n' '      sh_checker_exclude: >-' | parse_exclusions)
if exclusions_unreadable "$block_probe"; then
    printf '  ok       a block-scalar exclusion is refused, not scanned\n'
else
    printf '  FAIL     a block-scalar exclusion reads as: %s\n' "${block_probe:-nothing}"
    failed=1
fi

# A plain scalar continues onto an indented next line with nothing on
# its first line to say so: `sh_checker_exclude: ani-cli` continued
# by an indented `tests/arch` resolves to both tokens while a
# line-oriented read keeps the fragment. No spelling of the bare form
# is safe to read, so the readable set narrows to the quoted
# single-line forms and the bare form arrives as a refusal.
plain_probe=$(printf '%s\n' '      sh_checker_exclude: ani-cli' |
    parse_exclusions)
if exclusions_unreadable "$plain_probe"; then
    printf '  ok       a bare exclusion list is refused, not scanned\n'
else
    printf '  FAIL     a bare exclusion list reads as: %s\n' "${plain_probe:-nothing}"
    failed=1
fi

# YAML also continues a quoted scalar onto the next physical line:
# `sh_checker_exclude: "ani-cli` with the rest beneath it resolves to
# one list, while a line-oriented read of the first line extracts a
# valid-looking fragment — a token list missing exactly the entry
# that mattered, and nothing in it for a refusal to catch. A quote
# that opens on the line and never closes means the value is not on
# the line.
unterminated_probe=$(printf '%s\n' '      sh_checker_exclude: "ani-cli' |
    parse_exclusions)
if exclusions_unreadable "$unterminated_probe"; then
    printf '  ok       an unterminated quoted exclusion is refused, not scanned\n'
else
    printf '  FAIL     an unterminated quoted exclusion reads as: %s\n' \
        "${unterminated_probe:-nothing}"
    failed=1
fi

# Double-quoted YAML resolves escapes: "tests\x2farch" reaches the
# action as tests/arch and excludes these scripts, while the
# extraction keeps the backslash and the token matches nothing — the
# same silent pass the block-scalar case closed, one spelling over.
# An escape is YAML the extraction does not resolve.
escape_probe=$(printf '%s\n' '      sh_checker_exclude: "ani-cli tests\x2farch"' |
    parse_exclusions)
if exclusions_unreadable "$escape_probe"; then
    printf '  ok       an escaped exclusion is refused, not scanned\n'
else
    printf '  FAIL     an escaped exclusion reads as: %s\n' "${escape_probe:-nothing}"
    failed=1
fi

if [ -f "$arch_lint_workflow" ]; then
    lint_excludes=$(grep 'sh_checker_exclude' "$arch_lint_workflow" | head -1 || true)
    excluded_tokens=$(printf '%s' "$lint_excludes" | parse_exclusions)

    # Refuse what cannot be read. Any spelling beyond the forms the
    # extraction understands is out of scope by the contract's
    # incompleteness rule — but it has to be refused, not scanned.
    if exclusions_unreadable "$excluded_tokens"; then
        printf '  FAIL     the exclusion list could not be read: %s\n' "$lint_excludes"
        failed=1
        excluded_tokens=''
    fi
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
else
    printf '  FAIL     no exclude list to read: the arch lint workflow does not exist\n'
    failed=1
fi

# Whether a check can be redirected by the environment it runs in.
#
# Two have been, for real: `REPO_ROOT` and `SKIP_NESTED` each arrived
# from an ordinary shell and silently changed what a check did while it
# still exited 0. That is the property worth holding.
#
# It is held by running the check rather than by reading it. The
# earlier form audited the source text for names that looked ambient,
# which meant deciding from `grep` and `awk` which `$NAME` is a read,
# which assignment owns it, and which text is code at all. That
# question needs a shell parser, and it was assembled one review round
# at a time: comments, then quoting, then heredocs, then heredoc
# delimiters, then several heredocs per command, then line
# continuations, then command-scoped assignment prefixes — each
# arriving only once the previous had shipped, and each a defect in the
# fix before it.
#
# Running the check asks the question directly. It cannot grow new
# rules, because it has none.
#
# What it gives up is naming the offending variable. What it gains is
# an answer about behaviour rather than about spelling, and a check
# whose own correctness is obvious. `HOME`, `PATH` and `TMPDIR` are
# left alone below because these scripts legitimately need them.
#
# The coverage is exactly the list below, and the report says so. A
# check gating on a name absent from it is invisible here — the
# hostile run inherits or omits that name exactly as the clean run
# does — and the contract requires an incomplete check to state its
# boundary rather than imply coverage it does not have. When a new
# generic name bites, the remedy is one line: add it.
hostile_env() {
    env \
        REPO_ROOT=/nonexistent-hostile-root \
        SKIP_NESTED=1 \
        GENERIC_GUARD=1 \
        ROOT=/nonexistent \
        DIR=/nonexistent \
        FILE=/nonexistent \
        DEBUG=1 VERBOSE=1 QUIET=1 FORCE=1 DRY_RUN=1 \
        "$@"
}

# The clean runs shed the same names hostile_env sets — the two lists
# are the same list, name for name. Without this, a caller already
# exporting one of them poisons the baseline: every run sees the name,
# nothing differs, and the sensitivity vanishes. The hostile run needs
# no unsetting; it overrides each name explicitly.
clean_env() {
    env \
        -u REPO_ROOT \
        -u SKIP_NESTED \
        -u GENERIC_GUARD \
        -u ROOT \
        -u DIR \
        -u FILE \
        -u DEBUG -u VERBOSE -u QUIET -u FORCE -u DRY_RUN \
        "$@"
}

# Sensitive when a hostile environment changes either what the check
# prints or how it exits.
#
# A clean run happens twice first, to ask whether the check is
# reproducible against itself at all. `node_tool_tests.sh` is not — it
# prints durations — and comparing its output to anything reports a
# difference every time. For those the exit status is the only stable
# signal, so that is what gets compared. Found by this case on its
# first run, which is the sort of thing the text audit could never
# have seen.
env_sensitive() {
    _first=$(clean_env sh "$1" 2>&1) && _first_status=0 || _first_status=$?
    _again=$(clean_env sh "$1" 2>&1) || true
    _dirty=$(hostile_env sh "$1" 2>&1) && _dirty_status=0 || _dirty_status=$?

    [ "$_first_status" = "$_dirty_status" ] || return 0
    if [ "$_first" = "$_again" ] && [ "$_first" != "$_dirty" ]; then
        return 0
    fi
    return 1
}

# The hunt has to find something it is known to find, or the day it
# stops working it reports ok about every check at once.
if env_sensitive "$REPO_ROOT/tests/fixtures/arch/redirectable-check.sh"; then
    printf '  ok       a check a stray environment redirects is detected\n'
else
    printf '  FAIL     a check that reads SKIP_NESTED was not detected as sensitive\n'
    failed=1
fi

# The stray environment the hunt exists to catch can just as easily be
# the one this suite itself runs under. A caller that already exports
# a hostile name hands it to the clean runs too: all three runs are
# then redirected alike, the difference vanishes, and a sensitive
# check reads as clean. The clean baseline has to shed the hostile
# names, not merely differ from them.
if (
    SKIP_NESTED=1
    export SKIP_NESTED
    env_sensitive "$REPO_ROOT/tests/fixtures/arch/redirectable-check.sh"
); then
    printf '  ok       an exported hostile name cannot poison the clean baseline\n'
else
    printf '  FAIL     a caller exporting SKIP_NESTED=1 hides the sensitivity the calibration proves detectable\n'
    failed=1
fi

# Every real check, run twice. The self-tests are skipped: they
# re-execute themselves, and several deliberately vary on exactly the
# environment this hands them.
env_strays=''
for env_check in "$REPO_ROOT"/tests/arch/*.sh; do
    case "$(basename "$env_check")" in
        run-all.sh | *_test.sh) continue ;;
        *) ;;
    esac
    if env_sensitive "$env_check"; then
        env_strays="$env_strays $(basename "$env_check")"
    fi
done
if [ -z "$env_strays" ]; then
    printf '  ok       no check varies under the hostile names this run injects\n'
else
    printf '  FAIL     a stray environment redirects these checks:%s\n' "$env_strays"
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
    # The workflow is a subject too, now that cases read its trigger
    # and exclude list. Without this the nested run judges the
    # committed copy while the parent judges the tree, and they
    # disagree exactly when the tree is what changed.
    mkdir -p "$apostrophe_dir/repo/.github/workflows"
    cp "$REPO_ROOT/.github/workflows/arch-lint.yml" \
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
    ARCH_DEFERRAL_RECORD_LIB=1 ARCH_REPO_ROOT="$probe_repo" \
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
# The protocol first: every record the probe emits has to be spelled
# from an alphabet this run controls. An absolute path carries the
# checkout prefix, and a newline anywhere in that prefix splits one
# record into several — the reader then matches nothing and reports a
# failure about serialization, not about cleanup.
leak_protocol_ok=1
while IFS= read -r probe_line; do
    [ -n "$probe_line" ] || continue
    case "$probe_line" in
        */*) leak_protocol_ok=0 ;;
        *) ;;
    esac
done <"$leak_out"
if [ "$leak_protocol_ok" -eq 1 ]; then
    printf '  ok       the leak probe reports names, not paths\n'
else
    printf '  FAIL     the leak probe serializes absolute paths — a newline in the checkout path splits its records\n'
    failed=1
fi

# Verify the allocations exist while the child still holds them.
# Counting plausible-looking lines is not evidence: two fabricated
# paths, or the same one twice, satisfy any count. A path that is on
# disk now was really allocated, and checking before the signal is the
# only moment that is true.
leak_present=0
leak_seen=""
while IFS= read -r probe; do
    case "$probe" in
        .deferral-root.*.probe.*) ;;
        *) continue ;;
    esac
    case "$leak_seen" in
        *"[$probe]"*) continue ;;
        *) leak_seen="${leak_seen}[$probe]" ;;
    esac
    [ -e "$REPO_ROOT/tests/arch/$probe" ] && leak_present=$((leak_present + 1))
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
        .deferral-root.*.probe.*) ;;
        *) continue ;;
    esac
    leak_count=$((leak_count + 1))
    if [ -e "$REPO_ROOT/tests/arch/$probe" ]; then
        leak_found=1
        rm -f "$REPO_ROOT/tests/arch/$probe"
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

        # `test -e` follows the link and answers about the target, so a
        # dangling symlink reads as a free path. It survives the
        # refusal, fails the `mkdir`, and is then removed by the
        # cleanup the refusal exists to prevent — the one shape of
        # collision the check cannot see.
        dangling="$scratch_dir/dangling-prefix"
        ln -s /definitely/not/here "$dangling"
        dangling_status=0
        ARCH_DEFERRAL_TMP_PREFIX="$dangling" \
            sh "$REPO_ROOT/tests/arch/deferral_root_test.sh" >/dev/null 2>&1 ||
            dangling_status=$?
        if [ "$dangling_status" -ne 0 ] && [ -L "$dangling" ]; then
            printf '  ok       a dangling symlink at the prefix is refused, not removed\n'
        else
            printf '  FAIL     a dangling symlink at the prefix was adopted or removed (status %s, link %s)\n' \
                "$dangling_status" \
                "$([ -L "$dangling" ] && echo present || echo gone)"
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
