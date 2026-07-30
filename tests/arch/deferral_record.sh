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
    # Tracked is the whole question. `git check-ignore` answers a
    # narrower one — it is nonzero for a path that is merely absent,
    # so an ignore-based check waves through a record nobody added.
    # `ls-files --error-unmatch` fails for ignored, absent, and
    # untracked-on-disk alike, which are the same thing to someone
    # reading from a fresh clone. It also correctly accepts a
    # force-added file: tracked beats ignored, and such a file really
    # is readable.
    git ls-files --error-unmatch "$1" >/dev/null 2>&1
}

# A specific reason for the failure message, so the fix is obvious
# from the suite output rather than requiring a bisect.
why_unrecoverable() {
    if git check-ignore -q "$1" 2>/dev/null; then
        printf 'which git ignores'
    else
        printf 'which is not tracked by git'
    fi
}

# Backticked tokens from the section body (on stdin) that name a repo
# path.
#
# A token is a path if it has a slash or a dot: `docs/notes.md`,
# `.planning/follow-ups`, `docs/notes/`, and a bare `follow-ups.md` at
# the repository root all qualify. Every narrowing tried here — an
# extension, then a slash — turned out to exclude a real record, and
# each did so by dropping the token before the predicate ran, which
# surfaces as the invariant passing. A check wrong in that direction
# says nothing at all while looking like it said yes.
#
# So this errs toward over-matching. A false positive fails loudly and
# gets fixed; a false negative is invisible. The exclusions are
# carried by the character class — `#N · Title` and `git check-ignore`
# hold characters and spaces it does not admit, a URL is out on its
# colon, and a bare prose word has neither slash nor dot. All four are
# asserted in the self-test, so widening cannot quietly reclassify
# prose as paths.
cited_paths() {
    grep -o '`[^`]*`' \
        | tr -d '`' \
        | grep -E '^\.?[A-Za-z0-9_-]+(\.[A-Za-z0-9_-]+)*(/[A-Za-z0-9_.-]*)*$' \
        | grep -E '/|\.' \
        | sort -u
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
for p in $paths; do
    if ! record_is_recoverable "$p"; then
        printf 'arch/deferral_record: AGENTS.md tells agents to record deferred work in `%s`, %s — another checkout cannot read it\n' "$p" "$(why_unrecoverable "$p")"
        failed=1
    fi
done

if [ "$failed" -ne 0 ]; then
    printf 'arch/deferral_record: FAILED\n'
    exit 1
fi

printf 'arch/deferral_record: ok (%s tracked path(s) cited)\n' "$(printf '%s' "$paths" | grep -c . || true)"
