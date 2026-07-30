#!/bin/sh
# Architectural invariant: a deferral AGENTS.md tells you to record
# must land somewhere another contributor can actually read.
#
# The rule about not dropping work silently is only worth anything if
# the thing it points you at survives leaving your checkout. When the
# section was first written it named `.planning/follow-ups.md`, which
# `.gitignore` excludes wholesale — so a thread reply citing an entry
# there pointed at a file nobody else has. The deferral looked
# recorded and was, in practice, still lost.
#
# Concrete check: every backticked path inside the deferral section of
# AGENTS.md is tracked by git. `git check-ignore` is the authority,
# since it answers for the real ignore rules rather than for a
# hardcoded list of directories this script would have to keep in
# sync.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Can a contributor who clones this repo read the file at `$1`?
#
# NOTE: this is the check under test in deferral_record_test.sh.
record_is_recoverable() {
    # `--` and `:(literal)` because a declared path is data and git
    # would otherwise read parts of it as syntax. `--stage` is a real
    # `ls-files` flag, so without the separator the query lists the
    # whole index and an absent record reads as present; `[A]GENTS.md`
    # is a pathspec matching the tracked `AGENTS.md`, so without the
    # literal magic a record is reported readable because a different
    # file is. Both are legal filenames. Verified that the pair leaves
    # the other declared spellings working — a directory, a `./`
    # prefix, a name containing a space.
    #
    # Tracked is the whole question. `git check-ignore` answers a
    # narrower one — it is nonzero for a path that is merely absent,
    # so an ignore-based check waves through a record nobody added.
    # `ls-files --error-unmatch` fails for ignored, absent, and
    # untracked-on-disk alike, which are the same thing to someone
    # reading from a fresh clone. It also correctly accepts a
    # force-added file: tracked beats ignored, and such a file really
    # is readable.
    git ls-files --error-unmatch -- ":(literal)$1" >/dev/null 2>&1
}

# A specific reason for the failure message, so the fix is obvious
# from the suite output rather than requiring a bisect.
why_unrecoverable() {
    # `--` but not `:(literal)` here. check-ignore matches the name it
    # is given against the ignore rules rather than against the tree,
    # so the separator is enough to keep an option name out of the
    # argument list, while the literal magic stops it matching at all
    # and turns every ignored record into the vaguer "not tracked".
    if git check-ignore -q -- "$1" 2>/dev/null; then
        printf 'which git ignores'
    else
        printf 'which is not tracked by git'
    fi
}

# Backticked tokens from the section body (on stdin) that name a repo
# path.
#
# Paths the section declares for checking, one per marker line,
# taken verbatim.
#
# Not inferred from backticks. Six spellings of a legal record were
# dropped by successive attempts to recognise one by shape — an
# extension, a slash, a leading dot, a `./` prefix, then a name with
# neither slash nor dot, then one containing a space. Git constrains a
# path almost not at all, so the set of shapes is not enumerable and
# every miss failed silently, dropping the token before the predicate
# ran. A guess that cannot terminate is the wrong mechanism, however
# many times it is refined.
#
# So the section says which paths it means:
#
#     <!-- record-path: docs/follow-ups.md -->
#
# The rest of the line after the prefix is the path, spaces and all,
# up to the closing marker. Declaring is a little more typing and it
# is exact, which is the trade this check has already paid for six
# times over.
cited_paths() {
    sed -n 's/^[[:space:]]*<!-- record-path:[[:space:]]*\(.*[^[:space:]]\)[[:space:]]*-->[[:space:]]*$/\1/p'
}

# An untracked file for the self-test to point the predicate at.
# Echoes the path it created.
#
# `mktemp` rather than a fixed name: a developer with their own file
# at the fixed path would have it truncated here and deleted at
# cleanup. A check may not cost more than the defect it detects, and
# losing someone's uncommitted work to prove a point about
# uncommitted work would be a poor trade.
make_untracked_probe() {
    mktemp "${1:-$REPO_ROOT/tests/arch}/.deferral-probe.XXXXXX"
}

# Sourced as a library by the self-test — define the helpers, run
# nothing. Mirrors the __ANI_CLI_LIB__ guard the vendored script uses
# so its functions can be exercised directly.
if [ "${__DEFERRAL_RECORD_LIB__:-}" = "1" ]; then
    return 0
fi

SECTION='Scope is negotiable, delivery is not'

if ! grep -q "$SECTION" AGENTS.md; then
    printf 'arch/deferral_record: section "%s" not in AGENTS.md\n' "$SECTION"
    exit 1
fi

# The section body: from its heading to the next heading or EOF.
body=$(awk -v want="$SECTION" '
    /^## / { inside = index($0, want) > 0; next }
    inside { print }
' AGENTS.md)

paths=$(printf '%s\n' "$body" | cited_paths)

failed=0
# `read -r` line by line, because a declared path may contain spaces
# that a `for` over an unquoted expansion would tear in half. Fed by a
# here-document rather than a pipe: a pipeline would run the loop in a
# subshell and `failed` would not survive it, which is the same
# silent-pass failure in a different costume.
while IFS= read -r p; do
    [ -n "$p" ] || continue
    if ! record_is_recoverable "$p"; then
        printf 'arch/deferral_record: AGENTS.md tells agents to record deferred work in `%s`, %s — another checkout cannot read it\n' "$p" "$(why_unrecoverable "$p")"
        failed=1
    fi
done <<EOF
$paths
EOF

if [ "$failed" -ne 0 ]; then
    printf 'arch/deferral_record: FAILED\n'
    exit 1
fi

printf 'arch/deferral_record: ok (%s tracked path(s) cited)\n' "$(printf '%s' "$paths" | grep -c . || true)"
