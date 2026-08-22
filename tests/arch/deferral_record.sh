#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC2016 — single quotes are deliberate here — these lines print a
#       literal `$` or backtick
#   SC2310 — functions are called in `if` and `!` conditions on
#       purpose, so a failing check reports rather than aborting the run
#   SC2312 — command substitutions are read for their text, and a
#       failure arrives as an empty result that the assertion then catches
#
# Scoped to this file rather than widened in SHELLCHECK_OPTS, so the
# relaxation stays with the reporting scripts it is argued for.
# shellcheck disable=SC2016,SC2310,SC2312
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

# Overridable so the self-test can point the check at a scratch
# repository. It matters when this file is sourced as a library too:
# without the override it would re-derive its own root and `cd` there,
# silently undoing the caller's choice of repository.
REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$REPO_ROOT"

# Can a contributor who clones this repo read the file at `$1`?
#
# NOTE: this predicate is the subject under test in
# tests/bash/arch/deferral_record_predicate.bats and
# tests/bash/arch/deferral_record_entry_kind.bats.
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
    git ls-files --error-unmatch -- ":(literal)$1" >/dev/null 2>&1 || return 1

    # Tracked is necessary but not sufficient: the entry has to be
    # the record, not a pointer to one. A symlink (120000) and a
    # gitlink (160000) both have index entries and neither carries
    # content — the link may leave the repository or dangle, and a
    # submodule is an empty directory until initialised. Requiring a
    # regular blob (100644 / 100755) is stricter than checking where a
    # link goes, and deliberately so: a symlink to a tracked file
    # would be readable and is refused anyway, which fails loudly and
    # is the direction this check errs in everywhere else.
    # Every matched entry, not any of them. A declared directory is one
    # record and the pathspec then matches everything under it, so
    # asking whether *some* entry is a regular blob passes a directory
    # holding one real file beside a link that points nowhere. Asking
    # whether *no* entry fails the mode test is the same question for
    # a single file and the right one for a directory.
    if git ls-files --stage -- ":(literal)$1" 2>/dev/null |
        grep -qvE '^100(644|755) '; then
        return 1
    fi

    # `git add -N` records only the intent to add: a regular mode and
    # the empty blob, which every check above accepts, while
    # `write-tree` omits the path so no clone carries it. Porcelain
    # tells them apart — " A" for intent-to-add, "A " for staged.
    ! git status --porcelain -- ":(literal)$1" 2>/dev/null | grep -q '^ A'
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
    elif git ls-files --error-unmatch -- ":(literal)$1" >/dev/null 2>&1; then
        # Tracked, and still refused — so it is the wrong kind of
        # entry rather than a missing one. Saying "not tracked" here
        # would send its author to `git add` a path git already has.
        printf 'which is tracked as a symlink or submodule rather than a file'
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
# The format is exact: the line starts at column zero, one space after
# the colon, one space before the closing marker, and everything
# between them is the path.
#
# Column zero because four spaces of indentation makes a line an
# indented code block in Markdown, where a marker is inert — and
# picking the exact boundary is the habit that produced four rounds of
# fence rules. Anything indented is an example. A constraint that
# cannot drift, rather than a judgement that can. That single
# space on each side is the delimiter, so padding and content stay
# distinguishable — `<!-- record-path: notes.md  -->` declares
# `notes.md ` with its trailing space intact, which is a legal
# filename, rather than quietly checking `notes.md` instead and
# passing because some other file exists.
#
# Trimming would have been the easier read and is what a
# whitespace-tolerant pattern does. It substitutes a nearby filename
# for the declared one, which is the same silent pass this file exists
# to prevent — declaring a path is worth nothing if the parser then
# adjusts it.
cited_paths() {
    sed -n 's/^<!-- record-path: \(.*\) -->[[:space:]]*$/\1/p'
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
if [ "${ARCH_DEFERRAL_RECORD_LIB:-}" = "1" ]; then
    return 0
fi

SECTION='Scope is negotiable, delivery is not'

# The contract file, overridable so the self-test can drive this
# against a fixture instead of mutating the real one.
AGENTS_FILE="${1:-$REPO_ROOT/AGENTS.md}"

# The heading, exactly — `## 14. Scope is negotiable, delivery is not`
# — with the number optional so renumbering the document does not break
# this. Substring matching would adopt any heading containing the
# phrase and concatenate its body with the real one, so a marker in a
# lookalike section could stand in for the policy's own.
#
# Exactly one, too. Two matching headings join their bodies for the
# same reason, and one covering for the other is the failure being
# guarded against rather than a redundancy worth allowing.
# `[.]` rather than an escaped dot: awk warns on `\.` in a dynamic
# regex and falls back to a plain `.`, which matches any character —
# so `## 14X Scope ...` would have counted. A character class says the
# same thing with no escaping to be dropped.
SECTION_RE="^## ([0-9]+[.] )?$SECTION\$"
# Counted by line, without regard to what Markdown would render.
#
# This check establishes that the policy's declaration is present,
# unique and readable. It does not establish that the document renders
# as intended, and it no longer pretends to: parsing structure to find
# out whether a heading is "live" took five rounds of fence rules plus
# an indented-H2 rule, two of them fixing defects the parsing itself
# introduced, and the set of ways Markdown can render a line inert —
# fences, HTML comments, blockquotes, indented code — has no end.
#
# Uniqueness does the work instead. A heading or marker duplicated in
# an inert position makes two, and two fails, whatever construct did
# the inerting. What is left uncovered is a policy that exists *only*
# inert — the whole section fenced or commented out — which is visible
# to anyone opening the file and to any reviewer of the diff that did
# it. A parser is not the right instrument for that.
headings=$(grep -cE "$SECTION_RE" "$AGENTS_FILE" || true)

if [ "$headings" -eq 0 ]; then
    printf 'arch/deferral_record: no heading "## N. %s" in %s\n' "$SECTION" "$AGENTS_FILE"
    exit 1
fi
if [ "$headings" -ne 1 ]; then
    printf 'arch/deferral_record: %s headings match "%s" in %s — exactly one section may carry the policy, or a marker in either can stand in for the other\n' "$headings" "$SECTION" "$AGENTS_FILE"
    exit 1
fi

# The section body: from its heading to the next heading or EOF.
body=$(awk -v re="$SECTION_RE" '
    /^ {0,3}#{1,6} / { inside = ($0 ~ re); next }
    inside { print }
' "$AGENTS_FILE")

# The section may not contain a fenced block. A marker inside an
# example is inert but indistinguishable from a live one by line
# matching, and it would hold the at-least-one guard open after the
# real marker was deleted — the check passing while examining an
# example instead of a record.
#
# Refusing fences is the same answer CLAUDE.md's import got, and for
# the same reason: deciding which lines are live means implementing
# Markdown, and three rounds of that on the import proved the rules
# arrive faster than they can be learned. Scoped to this section, so
# the rest of AGENTS.md keeps its fences.
# `record-path` may appear exactly once in the file. An example
# anywhere — fenced, indented, quoted, or mid-sentence — is a second
# occurrence and fails, which takes the marker out of the fence
# question rather than adding another rule to the tracker.
mentions=$(grep -c 'record-path' "$AGENTS_FILE" || true)
if [ "$mentions" -ne 1 ]; then
    printf 'arch/deferral_record: `record-path` appears %s times in %s — exactly one line may mention it, so an example cannot stand in for the declaration\n' "$mentions" "$AGENTS_FILE"
    failed_fence=1
elif printf '%s\n' "$body" | grep -qE '^[[:space:]]*(```|~~~)'; then
    printf 'arch/deferral_record: the section contains a fenced block — it may not, because a marker inside an example cannot be told from a live declaration\n'
    failed_fence=1
else
    failed_fence=0
fi

paths=$(printf '%s\n' "$body" | cited_paths)

failed=$failed_fence
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

declared=$(printf '%s' "$paths" | grep -c . || true)

# Zero declarations is not a pass. The section exists to say where a
# deferral is recorded, so a section that names nothing has either
# lost its marker or malformed it — and in both cases the loop above
# runs zero times and finds nothing wrong, which would report success
# while checking nothing at all. An invariant that can be switched off
# by deleting a line, silently, is worse than no invariant, because
# the green is still there to be trusted.
if [ "$declared" -eq 0 ]; then
    printf 'arch/deferral_record: the section declares no record — expected at least one `<!-- record-path: ... -->` marker; a deleted or malformed one would disable this check without failing it\n'
    failed=1
fi

if [ "$failed" -ne 0 ]; then
    printf 'arch/deferral_record: FAILED\n'
    exit 1
fi

printf 'arch/deferral_record: ok (%s tracked path(s) cited)\n' "$declared"
