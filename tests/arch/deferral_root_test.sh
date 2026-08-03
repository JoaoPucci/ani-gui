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
# unterminated form is unreadable. It also covers what continues or
# indirects: a bare value that is a strict space-broken prefix of the
# name (or empty) is the first physical line of a folded spelling
# that resolves to the name, and a value opening with an anchor,
# alias or tag resolves somewhere a line count never looks.
count_arch_lint_names() {
    # The key side accepts the spellings that resolve to `name`: the
    # bare form, both quoted forms, and — refusal by over-count — any
    # double-quoted key carrying a backslash, since escapes can spell
    # the key however they like. `[[:space:]]*:` because YAML permits
    # whitespace between the key and its colon. The trailing
    # alternative counts explicit-form lines (`? key` / `: value`),
    # where key and value never share a physical line; counting both
    # halves over-counts one declaration, the failing-closed
    # direction.
    # An alias token directly before the colon can stand for any key
    # at all — `*k: value` resolves through an anchor this count does
    # not follow — so it joins the key alternation and the value side
    # decides, exactly as for the backslash-quoted key. An anchor
    # BEFORE a key (`&a name: v`) needs no arm: the key spelling
    # still appears on the line and matches its own alternative.
    grep -cE "(\"name\"|'name'|name|\"[^\"]*\\\\[^\"]*\"|\*[^:[:space:]]+)[[:space:]]*:[[:space:]]*(\"$arch_lint_name_re\"|'$arch_lint_name_re'|${arch_lint_name_re}[[:space:]]*([]#,}].*)?\$|[>|&*!]|\"[^\"]*\\\\[^\"]*\"|\"[^\"]*\$|'[^']*\$|(Arch( Shellcheck( \+)?)?)?[[:space:]]*\$|[\"']?\\\$[{][{].*${arch_lint_name_re})|^[[:space:]]*[?:]([[:space:]]|\$)" || true
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

# A plain scalar continues onto a more-indented next line and folds
# with a space: `name: Arch Shellcheck` over `+ Shfmt` resolves to
# exactly the required name while neither physical line spells it.
# Every folded spelling starts with a strict prefix of the name
# broken at a space — a finite set, derived from the constant above —
# so a bare name line spelling such a prefix counts as a potential
# producer: refusal by over-count, failing closed. `name:` with an
# empty value is the zero-length prefix of the same spelling, and a
# value opening with an anchor, alias or tag resolves through
# indirection this count does not follow, so those are refused the
# same way. None of these shapes appears in the live workflows, so
# the over-count costs nothing until someone writes one — and then it
# fails loudly instead of certifying.
folded_probe=$(printf 'jobs:\n  a:\n    name: Arch Shellcheck\n      + Shfmt\n' |
    count_arch_lint_names)
empty_probe=$(printf '%s\n' '    name:' | count_arch_lint_names)
alias_probe=$(printf '%s\n' '    name: *shared-name' | count_arch_lint_names)
tag_probe=$(printf '%s\n' "    name: !!str $arch_lint_name" |
    count_arch_lint_names)
if [ "${folded_probe:-0}" -ge 1 ] && [ "${empty_probe:-0}" -ge 1 ] &&
    [ "${alias_probe:-0}" -ge 1 ] && [ "${tag_probe:-0}" -ge 1 ]; then
    printf '  ok       a folded, indirect or empty name spelling is not silently missed\n'
else
    printf '  FAIL     folded/indirect name spellings count %s, %s, %s and %s — invisible to the scan\n' \
        "${folded_probe:-0}" "${empty_probe:-0}" "${alias_probe:-0}" "${tag_probe:-0}"
    failed=1
fi

# YAML permits whitespace between a key and its colon:
# `name : Arch Shellcheck + Shfmt` declares the same job name the
# unspaced spelling declares. A pattern requiring the colon to touch
# the key reads the spaced form as no declaration at all.
spaced_key_probe=$(printf '%s\n' "    name : $arch_lint_name" |
    count_arch_lint_names)
if [ "${spaced_key_probe:-0}" -eq 1 ]; then
    printf '  ok       a spaced key colon still counts as a producer\n'
else
    printf '  FAIL     a spaced key colon counts %s producers, not 1\n' \
        "${spaced_key_probe:-0}"
    failed=1
fi

# The key itself has spellings too. A quoted key resolves to the same
# mapping key the bare form declares, so `"name": ...` and
# `'name': ...` are the same declaration; a double-quoted key
# containing a backslash resolves through escapes this count does not
# interpret, so it could be `name` whatever it looks like; and the
# explicit form puts the key on a `?` line with the value on a `:`
# line beneath it, where no line carries both. The readable spellings
# count as the bare key does, and the unreadable ones count as
# potential producers — refusal by over-count, failing closed.
dq_key_probe=$(printf '%s\n' "    \"name\": $arch_lint_name" |
    count_arch_lint_names)
sq_key_probe=$(printf '%s\n' "    'name': $arch_lint_name" |
    count_arch_lint_names)
esc_key_probe=$(printf '%s\n' "    \"na\\u006de\": $arch_lint_name" |
    count_arch_lint_names)
explicit_key_probe=$(printf '    ? name\n    : %s\n' "$arch_lint_name" |
    count_arch_lint_names)
if [ "${dq_key_probe:-0}" -eq 1 ] && [ "${sq_key_probe:-0}" -eq 1 ] &&
    [ "${esc_key_probe:-0}" -ge 1 ] && [ "${explicit_key_probe:-0}" -ge 1 ]; then
    printf '  ok       a quoted, escaped or explicit key spelling is not silently missed\n'
else
    printf '  FAIL     quoted/escaped/explicit key spellings count %s, %s, %s and %s — invisible to the scan\n' \
        "${dq_key_probe:-0}" "${sq_key_probe:-0}" "${esc_key_probe:-0}" "${explicit_key_probe:-0}"
    failed=1
fi

# An alias can BE the key: with `&k name` anchored elsewhere,
# `*k: Arch Shellcheck + Shfmt` resolves to the same name property,
# and no physical line spells a recognized key. An alias token
# followed by a colon can stand for any key at all, so it counts
# whenever its value could be the required name — the same combining
# logic the backslash-quoted key uses: the key side says "possibly
# name", the value side decides. Both alias-key spacings count, since
# YAML accepts the colon adjacent or spaced.
alias_key_probe=$(printf '%s\n' "    *job_name_key: $arch_lint_name" |
    count_arch_lint_names)
alias_key_spaced_probe=$(printf '%s\n' "    *job_name_key : $arch_lint_name" |
    count_arch_lint_names)
if [ "${alias_key_probe:-0}" -ge 1 ] && [ "${alias_key_spaced_probe:-0}" -ge 1 ]; then
    printf '  ok       an aliased name key is not silently missed\n'
else
    printf '  FAIL     aliased name keys count %s and %s — invisible to the scan\n' \
        "${alias_key_probe:-0}" "${alias_key_spaced_probe:-0}"
    failed=1
fi

# GitHub evaluates expressions in names after YAML resolves:
# `name: "${{ 'Arch Shellcheck + Shfmt' }}"` reaches branch
# protection as exactly the required name, while the scalar the count
# reads is neither the literal nor any refused spelling. An
# expression value that textually carries the name counts as a
# potential producer. What this count deliberately does not attempt
# is evaluating expressions: a name constructed without its text
# appearing — format(), join(), an env lookup — is invisible to any
# line read, and that boundary is stated here rather than implied
# away. The live tree keeps expression names off the check-producing
# workflows, which is what the uniqueness case continues to certify.
# shellcheck disable=SC2016 # the unexpanded ${{ is the subject
expr_name_probe=$(printf 'name: "${{ %s }}"\n' "'$arch_lint_name'" |
    count_arch_lint_names)
if [ "${expr_name_probe:-0}" -ge 1 ]; then
    printf '  ok       an expression carrying the name is not silently missed\n'
else
    printf '  FAIL     an expression-valued name carrying the literal counts %s — invisible to the scan\n' \
        "${expr_name_probe:-0}"
    failed=1
fi

# Whether a workflow fires unconditionally: it exists and its trigger
# block names no paths, as a function so a fixture file runs through
# the same reading the live case runs. The trigger key has spellings
# — quoted forms and key-colon spacing resolve to the same key — so
# the range accepts them, and its end is any following top-level key
# whatever its spelling. A block that cannot be located at all reads
# as conditional: an empty range contains no `paths` for the wrong
# reason, and certifying from it is the failing-open direction.
unconditional() {
    [ -f "$1" ] || return 1
    _trigger=$(sed -nE "/^(\"on\"|'on'|on)[[:space:]]*:/,/^[^[:space:]#]/p" "$1")
    [ -n "$_trigger" ] || return 1
    # A key spelling inside the block that this reading cannot
    # resolve — an escape-bearing quoted key, an alias or anchor
    # token, the explicit form — reads as a filter being hidden, not
    # as its absence: the block is small and owned by this
    # repository, and every honest spelling it needs is readable.
    if printf '%s\n' "$_trigger" |
        grep -qE "\"[^\"]*\\\\[^\"]*\"[[:space:]]*:|^[[:space:]]*[*&][^:]*:|^[[:space:]]*[?:]([[:space:]]|\$)"; then
        return 1
    fi
    ! printf '%s\n' "$_trigger" | grep -q 'paths'
}

# A path filter would reopen both gaps at once: a pull request the
# filter misses never lints these scripts, and the mirror pair that
# papers over the zero-diff case is a second producer of the name
# again. No filter, no gap, no race.
if unconditional "$arch_lint_workflow"; then
    printf '  ok       the arch lint workflow fires unconditionally\n'
else
    printf '  FAIL     the arch lint workflow is missing or path-gated\n'
    failed=1
fi

# The trigger key has spellings too: `"on":` declares the same
# trigger the bare key declares, and a reading that recognizes only
# the bare spelling finds no block at all — an empty range contains
# no `paths`, so a path-gated workflow reads as unconditional, which
# is the failing-open direction. A trigger block that cannot be
# located must read as conditional.
quoted_on_gate="$scratch_dir/quoted-on-gate.yml"
printf '%s\n' '"on":' '  pull_request:' '    paths:' '      - "src/**"' \
    'permissions:' '  contents: read' >"$quoted_on_gate"
quoted_on_hidden=0
if unconditional "$quoted_on_gate"; then
    quoted_on_hidden=1
fi
if [ "$quoted_on_hidden" -eq 0 ]; then
    printf '  ok       a quoted trigger key cannot hide a path filter\n'
else
    printf '  FAIL     a path filter behind a quoted trigger key reads as unconditional\n'
    failed=1
fi
rm -f "$quoted_on_gate"

# The filter key inside the block has spellings too: a double-quoted
# key carrying an escape resolves to `paths` while spelling none of
# its letters where a literal search looks. The block is small and
# owned by this repository, so any key spelling the reading cannot
# resolve — an escape-bearing quoted key, an alias or anchor, the
# explicit form — reads as a filter being hidden, not as its absence.
escaped_gate="$scratch_dir/escaped-paths-gate.yml"
printf '%s\n' 'on:' '  pull_request:' '    "pa\x74hs":' \
    '      - "src/**"' 'permissions:' '  contents: read' >"$escaped_gate"
escaped_gate_hidden=0
if unconditional "$escaped_gate"; then
    escaped_gate_hidden=1
fi
if [ "$escaped_gate_hidden" -eq 0 ]; then
    printf '  ok       an escaped filter key cannot hide a path filter\n'
else
    printf '  FAIL     a path filter behind an escaped key reads as unconditional\n'
    failed=1
fi
rm -f "$escaped_gate"

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
# Which physical line of a workflow file carries the exclusion key,
# as a function so a fixture file runs through the same selection the
# live read runs. A declaration is the line where the key sits at the
# start, modulo indentation — a comment or any other mid-line mention
# is not one and must not shadow the line that is. The key has
# spellings of its own: both quoted forms resolve to the same key the
# bare form declares, a double-quoted key carrying a backslash can be
# that key whatever it looks like, and the explicit form (`? key` /
# `: value`) puts key and value on lines this read cannot join. All
# of them select and count — the quoted forms then refuse in
# parse_exclusions through their surviving key text, and the explicit
# halves each count, so the ambiguity refusal fires. YAML permits
# whitespace before the colon; the pattern tolerates it.
#
# One pattern for selection and count, so the two can never disagree
# about what a declaration is.
# The regex naming the action, shared by the file-wide count and the
# job-scoped one so the two can never disagree about what an
# invocation is.
# Anchored to a step-shaped line: `uses:` at the start of its line,
# optionally behind a list dash. A comment or any other mid-line
# mention is text about the action, not an invocation of it.
lint_action_re="^[[:space:]]*(-[[:space:]]+)?uses[[:space:]]*:[[:space:]]*['\"]?luizm/action-sh-checker"

# How many step lines invoke the sh-checker action. Carrying the
# required name is not linting: the job has to reach the action for
# anything to be inspected, and the count is a syntactic constraint —
# a `uses:` line naming the action either exists or does not.
lint_action_uses() {
    grep -cE "$lint_action_re" "$1" || true
}

# The remainder of the job that carries the required name: the lines
# from its bare name declaration to the next line at job-id
# indentation, or the end of the file. The same range family the
# trigger reading uses — an indentation constraint, not a parse. Two
# stated bounds: the segment starts at the first line spelling the
# bare name, so a job declaring its name after its steps reads as
# empty and refuses; a quoted or otherwise indirect name spelling is
# not found and refuses the same way. Both directions fail closed —
# the remedy is spelling the job the way the live workflow spells it.
required_job_segment() {
    awk -v name="name: $arch_lint_name" '
        found && /^  [^ ]/ { exit }
        found { print }
        index($0, name) { found = 1 }
    ' "$1" 2>/dev/null
}

# Whether the workflow actually lints where branch protection looks:
# within the job carrying the required name, exactly one invocation
# of the action and exactly one exclusion declaration. File-wide
# counts certify a workflow whose name-holding job inspects nothing
# while a second job lints somewhere branch protection never reads.
# Zero of either refuses as a job that starts, succeeds and inspects
# nothing; more than one is ambiguity the reads above already refuse.
lint_step_present() {
    _segment=$(required_job_segment "$1")
    [ -n "$_segment" ] || return 1
    [ "$(printf '%s\n' "$_segment" | grep -cE "$lint_action_re" || true)" -eq 1 ] &&
        [ "$(printf '%s\n' "$_segment" | grep -cE "$exclusion_key_re" || true)" -eq 1 ]
}

# The authority behind every text reading above: parse the workflows
# with a real parser and certify the RESOLVED structure, where
# quoting, escapes, folding, flow style, aliases and merge keys have
# already collapsed to the values GitHub sees. The text arms remain
# as the conservative belt — each fails closed on the spellings it
# names — but a spelling none of them names lands here, because
# resolution does not read spellings at all.
#
# Certified: exactly one job across the workflows resolves to the
# required name; its workflow's trigger carries no paths filter at
# any depth; the job invokes exactly one step whose action repository
# is exactly luizm/action-sh-checker; and that step's
# sh_checker_exclude input is a readable literal whose tokens do not
# cover tests/arch. Refused rather than certified: unparseable files,
# expression-valued names or exclusions, and a missing parser — a
# certification that cannot run must not read as one that passed.
#
# Stated boundaries: PyYAML resolves YAML 1.1, where a bare `on` key
# reads as boolean True — handled explicitly — and GitHub's parser
# differs at edges; a divergence surfaces as a refusal or a wrong
# producer count, never as a silent pass, because the certification
# demands exactly one well-formed producer.
certified_by_resolution() {
    python3 - "$1" "$arch_lint_name" <<'PYCERT'
import glob
import sys

try:
    import yaml
except Exception:
    print("resolution layer unavailable: PyYAML missing", file=sys.stderr)
    sys.exit(1)

wfdir, required = sys.argv[1], sys.argv[2]
SUBJECT = "tests/arch"
ACTION = "luizm/action-sh-checker"
producers = []
problems = []


def gated(node):
    if isinstance(node, dict):
        return any(
            key in ("paths", "paths-ignore") or gated(value)
            for key, value in node.items()
        )
    if isinstance(node, list):
        return any(gated(value) for value in node)
    return False


paths = sorted(glob.glob(wfdir + "/*.yml") + glob.glob(wfdir + "/*.yaml"))
if not paths:
    problems.append("no workflows to read")
for path in paths:
    try:
        with open(path, encoding="utf-8") as handle:
            docs = list(yaml.safe_load_all(handle))
    except Exception as exc:
        problems.append(f"{path}: unparseable: {exc}")
        continue
    for doc in docs:
        if not isinstance(doc, dict):
            continue
        jobs = doc.get("jobs")
        if jobs is None:
            continue
        if not isinstance(jobs, dict):
            problems.append(f"{path}: jobs is not a mapping")
            continue
        for jid, job in jobs.items():
            if not isinstance(job, dict):
                continue
            name = job.get("name")
            if isinstance(name, str) and "${{" in name:
                problems.append(f"{path}: job {jid}: expression-valued name refused")
                continue
            if name != required:
                continue
            producers.append((path, doc, jid, job))

if len(producers) != 1:
    problems.append(f"{len(producers)} resolved producers of the required name, not 1")
for path, doc, jid, job in producers:
    trigger = doc.get("on", doc.get(True))
    if trigger is None:
        problems.append(f"{path}: producer has no trigger")
    if gated(trigger):
        problems.append(f"{path}: producer trigger is path-filtered")
    steps = job.get("steps")
    if not isinstance(steps, list):
        problems.append(f"{path}: job {jid} has no steps")
        steps = []
    lint = []
    for step in steps:
        if not isinstance(step, dict):
            continue
        uses = step.get("uses")
        if isinstance(uses, str) and uses.split("@", 1)[0] == ACTION:
            lint.append(step)
    if len(lint) != 1:
        problems.append(f"{path}: job {jid} invokes the lint action {len(lint)} times, not 1")
        continue
    inputs = lint[0].get("with")
    if not isinstance(inputs, dict):
        problems.append(f"{path}: the lint step declares no inputs")
        continue
    excl = inputs.get("sh_checker_exclude", "")
    if not isinstance(excl, str) or "${{" in excl:
        problems.append(f"{path}: exclusion is not a readable literal")
        continue
    for token in excl.split():
        if SUBJECT == token or SUBJECT.startswith(token + "/"):
            problems.append(f"{path}: exclusion covers {SUBJECT} via {token!r}")

for problem in problems:
    print(problem, file=sys.stderr)
sys.exit(1 if problems else 0)
PYCERT
}

# The alias/anchor arm selects any line opening with `*` or `&` that
# carries a colon: `*k: "list"` resolves to a key this read cannot
# name, so it must be seen and then refused, while `&a key: value`
# anchors a key that parses normally once selected.
exclusion_key_re="^[[:space:]]*(\"sh_checker_exclude\"|'sh_checker_exclude'|sh_checker_exclude|\"[^\"]*\\\\[^\"]*\")[[:space:]]*:|^[[:space:]]*[?:]([[:space:]]|\$)|^[[:space:]]*[*&][^:]*:"

select_exclusion_line() {
    grep -E "$exclusion_key_re" "$1" | head -1 || true
}

# How many lines declare the key. One is a list; more than one is two
# exclude lists — a second sh-checker step carries its own — and
# reading the first says nothing about the second, so the live read
# refuses the count rather than picking a winner.
exclusion_declarations() {
    grep -cE "$exclusion_key_re" "$1" || true
}

parse_exclusions() {
    # The leading strip means an unextractable line reaches the
    # refusal with its first real character exposed — an alias or
    # anchor opener is then caught by the same arms that refuse those
    # openers in values. The strip's own substitution would satisfy
    # the first `t` and skip the second extraction, so a branch to
    # the next line clears the flag before the extractions run.
    sed "s/^[[:space:]]*//
t clear
: clear
s/.*sh_checker_exclude:[[:space:]]*\"\([^\"]*\)\".*/\1/; t
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
    # shellcheck disable=SC2016 # the unexpanded ${{ is the subject
    case "$1" in
        *sh_checker_exclude*) return 0 ;;
        '>'* | '|'* | '['* | '{'* | '&'* | '*'*) return 0 ;;
        *\\*) return 0 ;;
        *'${{'*) return 0 ;;
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

# A comment mentioning the key is not a declaration. Selected as one,
# it shadows the live line beneath it: the comment's tokens parse
# cleanly, the declaration that actually configures the action is
# never read, and the case reports the scripts linted while the live
# list excludes them. The declaration is the line where the key sits
# at the start, modulo indentation; a mention anywhere else on a line
# is commentary or someone else's value.
shadow_input="$scratch_dir/comment-shadow.yml"
printf '%s\n' \
    '          # sh_checker_exclude: "ani-cli"' \
    '          sh_checker_exclude: "ani-cli tests/arch"' \
    >"$shadow_input"
shadow_tokens=$(select_exclusion_line "$shadow_input" | parse_exclusions)
shadow_hit=0
shadow_subject=tests/arch
for token in $shadow_tokens; do
    case "$shadow_subject" in
        "$token" | "$token"/*) shadow_hit=1 ;;
        *) ;;
    esac
done
if [ "$shadow_hit" -eq 1 ]; then
    printf '  ok       a commented mention does not shadow the declaration beneath it\n'
else
    printf '  FAIL     a comment mentioning the key shadows the live exclusion list, which reads as: %s\n' \
        "${shadow_tokens:-nothing}"
    failed=1
fi
rm -f "$shadow_input"

# The exclusion key has quoted and explicit spellings, exactly as the
# name key does. `"sh_checker_exclude": "ani-cli tests/arch"` is a
# valid action input that resolves to the same key, but a selection
# reading only the bare spelling never sees it: the token list comes
# back empty, empty reads as excluding nothing, and the suite
# certifies scripts the action excludes. Seen, the quoted-key line
# refuses in the parse through its surviving key text — so the fix is
# to make the selection see it, and the refusal already waits behind
# it. The explicit form (`? key` / `: value`) is unreadable the same
# way the explicit name form is, and counts so the ambiguity refusal
# fires.
quoted_key_excl="$scratch_dir/quoted-key-excl.yml"
printf '%s\n' '          "sh_checker_exclude": "ani-cli tests/arch"' \
    >"$quoted_key_excl"
quoted_key_count=$(exclusion_declarations "$quoted_key_excl")
quoted_key_tokens=$(select_exclusion_line "$quoted_key_excl" | parse_exclusions)
explicit_key_excl="$scratch_dir/explicit-key-excl.yml"
printf '          ? sh_checker_exclude\n          : "ani-cli tests/arch"\n' \
    >"$explicit_key_excl"
explicit_excl_count=$(exclusion_declarations "$explicit_key_excl")
if [ "${quoted_key_count:-0}" -eq 1 ] &&
    exclusions_unreadable "$quoted_key_tokens" &&
    [ "${explicit_excl_count:-0}" -ge 1 ]; then
    printf '  ok       a quoted or explicit exclusion key is not silently missed\n'
else
    printf '  FAIL     quoted/explicit exclusion keys: %s declarations, tokens read as %s, explicit counts %s\n' \
        "${quoted_key_count:-0}" "${quoted_key_tokens:-nothing}" "${explicit_excl_count:-0}"
    failed=1
fi
rm -f "$quoted_key_excl" "$explicit_key_excl"

# An alias can be the exclusion key too: with `&k sh_checker_exclude`
# anchored elsewhere, `*k: "ani-cli tests/arch"` resolves to the
# expected input while no line spells the key. The declaration has to
# be seen — counted and selected — and then refused, because an alias
# key could stand for anything and the list behind it cannot be read
# from this line.
alias_key_excl="$scratch_dir/alias-key-excl.yml"
printf '%s\n' '          *exclude_key: "ani-cli tests/arch"' \
    >"$alias_key_excl"
alias_excl_count=$(exclusion_declarations "$alias_key_excl")
alias_excl_tokens=$(select_exclusion_line "$alias_key_excl" | parse_exclusions)
alias_excl_refused=0
if exclusions_unreadable "$alias_excl_tokens"; then
    alias_excl_refused=1
fi
if [ "${alias_excl_count:-0}" -ge 1 ] && [ "$alias_excl_refused" -eq 1 ]; then
    printf '  ok       an aliased exclusion key is seen and refused\n'
else
    printf '  FAIL     an aliased exclusion key counts %s declarations and its tokens read as %s\n' \
        "${alias_excl_count:-0}" "${alias_excl_tokens:-nothing}"
    failed=1
fi
rm -f "$alias_key_excl"

# The action evaluates expressions in its inputs: an exclusion spelled
# `"${{ 'ani-cli tests/arch' }}"` reaches the linter as the resolved
# list, while the extraction hands back the unresolved expression and
# its whitespace-split tokens match nothing. An expression is a value
# this read cannot resolve — refused, like every other spelling past
# the extraction's boundary.
# shellcheck disable=SC2016 # the unexpanded ${{ is the subject
expr_excl_probe=$(printf 'sh_checker_exclude: "${{ %s }}"\n' "'ani-cli tests/arch'" |
    parse_exclusions)
if exclusions_unreadable "$expr_excl_probe"; then
    printf '  ok       an expression-valued exclusion is refused, not scanned\n'
else
    printf '  FAIL     an expression-valued exclusion reads as: %s\n' \
        "${expr_excl_probe:-nothing}"
    failed=1
fi

# Two declarations are two exclude lists — a second sh-checker step
# carries its own — and reading the first says nothing about the
# second, which is exactly where an exclusion of these scripts would
# hide. The count is the fact the live read refuses on; more than one
# declaration is ambiguity, not a list.
ambiguous_input="$scratch_dir/ambiguous-excl.yml"
printf '%s\n' \
    '          sh_checker_exclude: "ani-cli"' \
    '          sh_checker_exclude: "tests/arch"' \
    >"$ambiguous_input"
ambiguous_count=$(exclusion_declarations "$ambiguous_input" 2>/dev/null || true)
if [ "${ambiguous_count:-0}" -eq 2 ]; then
    printf '  ok       a second exclusion declaration is counted, not skipped\n'
else
    printf '  FAIL     two exclusion declarations count as %s — the second is invisible\n' \
        "${ambiguous_count:-0}"
    failed=1
fi
rm -f "$ambiguous_input"

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

# The text readings above are one layer: conservative, fail-closed,
# and by construction incomplete — YAML's spelling space is unbounded
# and each reading covers the spellings it names. The authority is
# resolution: parse every workflow with a real parser and certify the
# RESOLVED structure, where quoting, escapes, folding, flow, aliases
# and merge keys have already collapsed to the values GitHub sees. A
# merge key is the probe because no text arm can see it: the required
# name arrives in a job through `<<: *defaults` while no line of the
# job spells a name at all.
merge_dir="$scratch_dir/merge-key-producer"
mkdir "$merge_dir"
cat >"$merge_dir/covert.yml" <<'YAML'
defs: &d
  name: Arch Shellcheck + Shfmt
"on": push
jobs:
  stub:
    <<: *d
    runs-on: ubuntu-latest
    steps:
      - run: echo done
YAML
merge_caught=0
if ! certified_by_resolution "$merge_dir" 2>/dev/null; then
    merge_caught=1
fi
live_certified=0
if certified_by_resolution "$REPO_ROOT/.github/workflows" 2>/dev/null; then
    live_certified=1
fi
if [ "$merge_caught" -eq 1 ] && [ "$live_certified" -eq 1 ]; then
    printf '  ok       the resolved workflow structure certifies what the text layer cannot\n'
else
    printf '  FAIL     resolution reads merge-key-caught=%s live-certified=%s — a resolved second producer is invisible\n' \
        "$merge_caught" "$live_certified"
    failed=1
fi
rm -rf "$merge_dir"

# Carrying the required name is not linting. A job that keeps the
# name but drops the sh-checker step satisfies branch protection
# while inspecting nothing, and with no exclusion declared the
# coverage case reads empty tokens as "nothing excluded" — an
# echo-only job certified as the lint. The workflow has to invoke the
# action, exactly once, and declare its exclude list, exactly once;
# zero of either is a workflow that no longer lints.
stripped_workflow="$scratch_dir/stripped-lint.yml"
printf '%s\n' 'on: push' 'jobs:' '  arch-sh-checker:' \
    "    name: $arch_lint_name" '    runs-on: ubuntu-latest' \
    '    steps:' '      - run: echo done' >"$stripped_workflow"
live_lints=0
if lint_step_present "$arch_lint_workflow" 2>/dev/null; then
    live_lints=1
fi
stripped_refused=1
if lint_step_present "$stripped_workflow" 2>/dev/null; then
    stripped_refused=0
fi
if [ "$live_lints" -eq 1 ] && [ "$stripped_refused" -eq 1 ]; then
    printf '  ok       the name-holding job invokes the lint action with its exclude list\n'
else
    printf '  FAIL     lint-step presence reads live=%s stripped-refused=%s — a name without the action certifies\n' \
        "$live_lints" "$stripped_refused"
    failed=1
fi
rm -f "$stripped_workflow"

# The counts have to hold within the job that carries the name, not
# across the file. Split them — an echo-only job holding the required
# name beside a second job invoking the action with the sole exclude
# list — and branch protection is satisfied by the echo job while the
# lint runs somewhere branch protection never looks. File-wide counts
# read one of each and certify it.
split_workflow="$scratch_dir/split-lint.yml"
printf '%s\n' 'on: push' 'jobs:' '  stub:' \
    "    name: $arch_lint_name" '    runs-on: ubuntu-latest' \
    '    steps:' '      - run: echo done' \
    '  real-lint:' '    runs-on: ubuntu-latest' '    steps:' \
    '      - uses: luizm/action-sh-checker@master' \
    '        with:' \
    '          sh_checker_exclude: "ani-cli"' >"$split_workflow"
split_refused=1
if lint_step_present "$split_workflow" 2>/dev/null; then
    split_refused=0
fi
if [ "$split_refused" -eq 1 ]; then
    printf '  ok       a name-holding job cannot borrow another job'"'"'s lint step\n'
else
    printf '  FAIL     an echo job with the name certifies on the strength of a different job'"'"'s lint\n'
    failed=1
fi
rm -f "$split_workflow"

# A mention is not a step. A comment spelling the action satisfies an
# unanchored match, and an env key can spell the input name without
# the action ever reading it: together they dress an echo-only job as
# the lint. The invocation has to be a step-shaped line — `uses:` at
# the start of its line, optionally behind a list dash — and a
# comment is then just a comment.
commented_workflow="$scratch_dir/commented-lint.yml"
printf '%s\n' 'on: push' 'jobs:' '  arch-sh-checker:' \
    "    name: $arch_lint_name" '    runs-on: ubuntu-latest' \
    '    env:' '      sh_checker_exclude: "ani-cli"' \
    '    steps:' \
    '      # uses: luizm/action-sh-checker@master' \
    '      - run: echo done' >"$commented_workflow"
commented_refused=1
if lint_step_present "$commented_workflow" 2>/dev/null; then
    commented_refused=0
fi
if [ "$commented_refused" -eq 1 ]; then
    printf '  ok       a commented action mention does not count as the lint step\n'
else
    printf '  FAIL     an echo job certifies on a commented uses line and an env key\n'
    failed=1
fi
rm -f "$commented_workflow"

if [ -f "$arch_lint_workflow" ]; then
    # A workflow that never reaches the action lints nothing, however
    # its exclude list reads. Required before the list is trusted.
    if lint_step_present "$arch_lint_workflow"; then
        printf '  ok       the workflow invokes the lint action with one exclude list\n'
    else
        printf '  FAIL     the workflow does not invoke the lint action with exactly one exclude list\n'
        failed=1
    fi
    lint_excludes=$(select_exclusion_line "$arch_lint_workflow")
    excluded_tokens=$(printf '%s' "$lint_excludes" | parse_exclusions)

    # Refuse what cannot be read. Any spelling beyond the forms the
    # extraction understands is out of scope by the contract's
    # incompleteness rule — but it has to be refused, not scanned.
    # More than one declaration is the same situation reached another
    # way: the list this read did not pick may be the one that
    # matters.
    declaration_count=$(exclusion_declarations "$arch_lint_workflow")
    if [ "${declaration_count:-0}" -gt 1 ]; then
        printf '  FAIL     %s lines declare the exclusion key — ambiguous, refusing to pick one\n' \
            "$declaration_count"
        failed=1
        excluded_tokens=''
    elif exclusions_unreadable "$excluded_tokens"; then
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
    _again=$(clean_env sh "$1" 2>&1) && _again_status=0 || _again_status=$?
    _dirty=$(hostile_env sh "$1" 2>&1) && _dirty_status=0 || _dirty_status=$?

    # Two clean runs disagreeing about their own exit means the check
    # has no stable signal at all — noise, reported as sensitive so a
    # human looks, never as a clean bill built on a coin flip.
    [ "$_first_status" = "$_again_status" ] || return 0
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

# A check whose exit status flaps between identical runs has no
# stable signal at all: its output repeats, so the reproducibility
# gate passes, and its status is then compared as though it meant
# something. The second clean run's status is measured for exactly
# this — two clean runs disagreeing about their own exit means the
# check is noise, and noise reads as sensitive so a human looks,
# rather than as a clean bill built on a coin flip.
flapping_check="$scratch_dir/flapping-check.sh"
cat >"$flapping_check" <<'FLAP'
#!/bin/sh
marker="$0.marker"
if [ -e "$marker" ]; then
    rm -f "$marker"
    exit 1
fi
: >"$marker"
exit 0
FLAP
flap_flagged=0
if env_sensitive "$flapping_check"; then
    flap_flagged=1
fi
rm -f "$flapping_check" "$flapping_check.marker"
if [ "$flap_flagged" -eq 1 ]; then
    printf '  ok       a status-flapping check is flagged, not certified\n'
else
    printf '  FAIL     a check whose exit flaps between clean runs reads as insensitive\n'
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
