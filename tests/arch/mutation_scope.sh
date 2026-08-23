#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC2312 — command substitutions are read for their text, and a
#       failure arrives as an empty result that the assertion catches
#
# shellcheck disable=SC2312
# Architectural invariant: the nightly mutation run is scoped to
# directories that exist.
#
# `cargo mutants -f <glob>` treats a glob that matches nothing as an
# empty selection rather than an error. The job succeeds, uploads its
# report, and the module the glob named contributes no mutants — which
# on the summary is indistinguishable from a module whose every mutant
# was caught. So renaming or deleting a directory silently narrows the
# scope, and the only trace is a count in an artifact nobody opens.
# `src/anicli/**/*.rs` outlived the directory it named, and the run
# went on passing.
#
# What is asserted is a syntactic constraint plus a stat: the `-f`
# values are single-quoted literals in the workflow, and the leading
# directory of each either exists under the crate or it does not.
# Nothing here reasons about what cargo-mutants would match.
#
# Deliberately not covered, and said so rather than left to be assumed:
#
#   - That the crate the globs resolve against really is `backend`.
#     The step's `working-directory:` is what makes them crate-relative,
#     and reading that back out means parsing YAML, so the root is a
#     parameter instead. A step moved to another crate would leave this
#     comparing new globs against the old tree.
#   - A `-f` line that is payload — inside a comment, or a block scalar
#     that only looks like an argument list. Telling those apart is the
#     same parse.
#
# Both need a reviewed workflow edit that spells out the mimicry, which
# is the thing review is for.

set -eu

REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$REPO_ROOT"

WORKFLOW="${1:-$REPO_ROOT/.github/workflows/mutation.yml}"
CRATE_ROOT="${2:-$REPO_ROOT/backend}"

if [ ! -f "$WORKFLOW" ]; then
    printf 'arch/mutation_scope FAIL: %s is missing — the run this check scopes does not exist\n' \
        "$WORKFLOW" >&2
    exit 1
fi

if [ ! -d "$CRATE_ROOT" ]; then
    printf 'arch/mutation_scope FAIL: %s is missing — the globs have no tree to resolve against\n' \
        "$CRATE_ROOT" >&2
    exit 1
fi

globs=$(grep -oE "^[[:space:]]*-f[[:space:]]+'[^']+'" "$WORKFLOW" |
    sed -e "s/^[[:space:]]*-f[[:space:]]*'//" -e "s/'\$//")

# With nothing to resolve, every workflow satisfies this check. That is
# an absence of evidence rather than a pass, and it has to read as one.
if [ -z "$globs" ]; then
    printf 'arch/mutation_scope FAIL: %s declares no -f globs — this check has lost its subject and would report ok about a scope it never read\n' \
        "$WORKFLOW" >&2
    exit 1
fi

failed=0
checked=0
while IFS= read -r glob; do
    [ -n "$glob" ] || continue
    checked=$((checked + 1))
    case "$glob" in
        *'*'*)
            # Everything ahead of the first wildcard is a literal path,
            # and it is the part a rename invalidates.
            prefix="${glob%%\**}"
            prefix="${prefix%/}"
            # A glob that starts with its wildcard names no directory
            # to stat; there is nothing for this check to say about it.
            [ -n "$prefix" ] || continue
            if [ ! -d "$CRATE_ROOT/$prefix" ]; then
                printf 'arch/mutation_scope FAIL: %s globs %s, but %s does not exist — cargo-mutants matches nothing there and the run passes having mutated it zero times\n' \
                    "$WORKFLOW" "$glob" "$CRATE_ROOT/$prefix" >&2
                failed=1
            fi
            ;;
        *)
            if [ ! -e "$CRATE_ROOT/$glob" ]; then
                printf 'arch/mutation_scope FAIL: %s names %s, but %s does not exist — the file contributes no mutants and the run passes without it\n' \
                    "$WORKFLOW" "$glob" "$CRATE_ROOT/$glob" >&2
                failed=1
            fi
            ;;
    esac
done <<EOF
$globs
EOF

[ "$failed" -eq 0 ] || exit 1
printf 'arch/mutation_scope: ok (%d globs, every one resolving inside %s)\n' \
    "$checked" "$CRATE_ROOT"
