#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC1091 — the sourced path is built at runtime, so it cannot be
#       followed statically
#   SC2016 — single quotes are deliberate here — these lines print a
#       literal `$` or backtick
#   SC2030 — the subshell-local assignment is the case being exercised
#   SC2031 — same subshell case as SC2030
#   SC2310 — functions are called in `if` and `!` conditions on
#       purpose, so a failing check reports rather than aborting the run
#   SC2312 — command substitutions are read for their text, and a
#       failure arrives as an empty result that the assertion then catches
#
# Scoped to this file rather than widened in SHELLCHECK_OPTS, which
# would also relax the checks guarding the `ani-cli` script itself.
# shellcheck disable=SC1091,SC2016,SC2030,SC2031,SC2310,SC2312
# Self-test for the deferral-record invariant.
#
# The check it guards makes a claim — "a contributor who clones this
# repo can read the cited record" — and there is more than one way for
# that to be false. A path git ignores is the obvious one. A path that
# is merely absent is the quiet one: nothing ignores `docs/notes.md`,
# so an ignore-based check calls it fine while every other checkout
# has no such file. Both are the same failure to a reader.
#
# So the predicate is exercised directly against all three states
# rather than inferred from whether the suite happens to be green.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Hand the resolved root to the library under the specific name. It
# deliberately ignores a bare `REPO_ROOT`, and its own `$0` is
# meaningless after the `cd` above — without this, an invocation by
# relative path from this directory resolves above the repository.
ARCH_REPO_ROOT="$REPO_ROOT"
export ARCH_REPO_ROOT

# The library reads this after being sourced. shellcheck cannot follow
# a source path built at runtime, so it sees an assignment and no use.
# The old spelling was exempt only because it led with underscores.
# shellcheck disable=SC2034
ARCH_DEFERRAL_RECORD_LIB=1
# shellcheck source=./deferral_record.sh
. "$REPO_ROOT/tests/arch/deferral_record.sh"
unset ARCH_DEFERRAL_RECORD_LIB

failed=0

# Anything the run creates goes here and is removed on every exit
# path, including an interrupt — a suite that leaves litter in the
# working tree makes the next `git status` lie.
# A directory rather than a list of paths: POSIX sh has no arrays, so
# an accumulator must join filenames with a separator, and any
# separator is a character some real path contains. A repository
# cloned under `~/My Repos/` defeats a space-joined one outright —
# nothing is removed and `rm -f` receives fragments. One quoted
# variable has no such failure mode.
#
# The name is a literal fixed before anything exists, so the cleanup
# never refers to something a creating command produced. `mktemp -d`
# cannot offer that under any ordering of statements: the directory is
# on disk the moment it returns and the variable holds the path only
# once the substitution completes, and a signal in between leaves it
# with nothing able to name it.
scratch_dir="$REPO_ROOT/tests/arch/.deferral-scratch.$$"

# Removal is gated on this run having made the directory. Arming
# before creating is what closes the leak; without the gate it opens
# the opposite hole, where a path taken by somebody else is removed on
# behalf of a run that never created it.
scratch_owned=""
cleanup() { [ -n "$scratch_owned" ] && rm -rf "$scratch_dir"; }

# EXIT owns cleanup; the signal handlers only have to end the run.
# Handling a signal without exiting returns control to the interrupted
# script, which then carries on against a directory it just deleted
# and can still reach `exit 0` — a cancelled CI job reporting a pass.
# The statuses are the conventional 128 + signal number.
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

# `test -e` is false for a dangling symlink, so a link left at this
# path would read as free, survive the check, and then be removed —
# the pre-existing thing this refuses to touch. `-L` asks the question
# `-e` cannot.
if [ -e "$scratch_dir" ] || [ -L "$scratch_dir" ]; then
    printf 'arch/deferral_record_test: %s already exists — refusing to reuse or remove a path this run did not create\n' \
        "$scratch_dir" >&2
    exit 1
fi
# Claimed before the directory is made, not after. Recorded afterwards
# there is a gap: a signal between the `mkdir` returning and the record
# being written finds the directory on disk and the cleanup unwilling
# to touch it, so a cancelled run litters the working tree — which is
# what this scratch exists to avoid.
#
# Claimed first, the gap closes. Ahead of the `mkdir` there is nothing
# at the path to remove: the refusal above established that, and no
# live process shares this one's pid. If the `mkdir` fails after all,
# the claim is given back before the run ends.
#
# What remains is a signal arriving while another process holds the
# path, in the instant between the refusal and the `mkdir`. That is
# narrower than the window it replaces, and this scratch sits inside
# the repository rather than in a world-writable directory, so the
# path is not one a stranger is placing bets on.
scratch_owned=1
mkdir "$scratch_dir" || {
    scratch_owned=""
    printf 'arch/deferral_record_test: %s was claimed while this run was starting\n' \
        "$scratch_dir" >&2
    exit 1
}
# Where the gap used to sit. Ownership is claimed before the `mkdir`,
# so a signal arriving here takes the directory with it.
if [ -n "${ARCH_DEFERRAL_PAUSE_AFTER_MKDIR:-}" ]; then
    sleep "$ARCH_DEFERRAL_PAUSE_AFTER_MKDIR"
fi

# Probe mode: the run re-executes itself with this set so the signal
# case can watch a real process take a real signal, rather than
# inspecting trap definitions and hoping they mean what they say.
if [ -n "${ARCH_DEFERRAL_SIGNAL_PROBE:-}" ]; then
    # The basename, not the path: the record crosses a pipe and the
    # spelling here is the only part of the path no checkout location
    # can put a delimiter into.
    printf '%s\n' "${scratch_dir##*/}"
    sleep 5
    exit 0
fi

expect_recoverable() {
    if record_is_recoverable "$1"; then
        printf '  ok       %s\n' "$1"
    else
        printf '  FAIL     %s — tracked, but the check rejected it\n' "$1"
        failed=1
    fi
}

expect_rejected() {
    if record_is_recoverable "$1"; then
        printf '  FAIL     %s — %s, but the check accepted it\n' "$1" "$2"
        failed=1
    else
        printf '  ok       %s (%s)\n' "$1" "$2"
    fi
}

# The scratch path has to be a literal fixed before anything exists.
# `mktemp -d` cannot be registered in advance under any ordering: the
# directory is on disk the moment it returns and the variable holds it
# only once the substitution completes, so a signal in between leaves
# it with nothing able to name it. Asserted over the path, because the
# ordering it stands for is not observable after the fact.
if [ "$scratch_dir" = "$REPO_ROOT/tests/arch/.deferral-scratch.$$" ]; then
    printf '  ok       the scratch path is a literal, not an allocation result\n'
else
    printf '  FAIL     the scratch path came back from an interruptible allocation: %s\n' \
        "$scratch_dir"
    failed=1
fi

# A run cancelled between making the scratch directory and recording
# that it owns it must still take the directory with it. Recording
# ownership afterwards leaves that window open, and a cancelled run
# then litters the working tree — the thing this scratch exists to
# avoid, failing in the one situation nobody watches.
#
# The window is too short to step into, so the run holds it open on
# request. Without that the case passes while exercising nothing: the
# child finishes before the signal lands.
if [ -z "${ARCH_DEFERRAL_PAUSE_AFTER_MKDIR:-}" ]; then
    # A sibling that merely shares the naming scheme is somebody
    # else's — a concurrent run's, or a developer's. A sweep that
    # removes every match destroys their state on the way to
    # reporting ours.
    # An occupant of the fixed spelling, planted before the case runs.
    # It is somebody's — the case may not create, write into, or
    # remove anything at this path, and the assertion at the end holds
    # that for every future edit of this block.
    gap_occupied="$REPO_ROOT/tests/arch/.deferral-scratch.sentinel"
    gap_occupied_planted=0
    if [ ! -e "$gap_occupied" ]; then
        mkdir "$gap_occupied"
        printf 'keep existing\n' >"$gap_occupied/keep-existing"
        gap_occupied_planted=1
    fi

    # Allocated, not spelled: a fixed name is somebody's the moment
    # anybody else uses it. `mktemp -d` hands back a directory nobody
    # holds, still matching the naming scheme the case is about.
    gap_sentinel=$(mktemp -d "$REPO_ROOT/tests/arch/.deferral-scratch.XXXXXX")
    printf 'not yours\n' >"$gap_sentinel/keep-me"

    ARCH_DEFERRAL_PAUSE_AFTER_MKDIR=3 \
        sh "$REPO_ROOT/tests/arch/deferral_record_test.sh" >/dev/null 2>&1 &
    gap_pid=$!
    # The child names its scratch from its own pid, which is the pid
    # just captured — so the one path this case may touch is known
    # exactly, and nothing that merely shares the naming scheme is.
    gap_dir="$REPO_ROOT/tests/arch/.deferral-scratch.$gap_pid"

    # Observed to exist before the signal, or the case has nothing to
    # measure: a child that dies before allocating leaves no
    # directory, and "no directory afterwards" would then read as
    # cleanup having worked.
    gap_seen=0
    i=0
    while [ "$i" -lt 50 ]; do
        if [ -d "$gap_dir" ]; then
            gap_seen=1
            break
        fi
        sleep 0.1
        i=$((i + 1))
    done

    kill -TERM "$gap_pid" 2>/dev/null || true
    gap_status=0
    wait "$gap_pid" || gap_status=$?

    if [ "$gap_seen" -eq 0 ]; then
        printf '  FAIL     the ownership-gap child never reached its scratch (exit %s)\n' \
            "$gap_status"
        failed=1
    elif [ "$gap_status" -ne 143 ]; then
        printf '  FAIL     the ownership-gap child exited %s, not 143 — it did not die of the signal\n' \
            "$gap_status"
        failed=1
        rm -rf "$gap_dir"
    elif [ -d "$gap_dir" ]; then
        printf '  FAIL     a cancelled run left its scratch directory behind\n'
        failed=1
        rm -rf "$gap_dir"
    else
        printf '  ok       a run cancelled before recording ownership takes its scratch with it\n'
    fi

    if [ -f "$gap_sentinel/keep-me" ]; then
        printf '  ok       a sibling sharing the naming scheme survives the case\n'
    else
        printf '  FAIL     the case swept a scratch directory it did not create\n'
        failed=1
    fi
    rm -rf "$gap_sentinel"

    if [ -f "$gap_occupied/keep-existing" ]; then
        printf '  ok       an occupant of the fixed spelling survives the whole case\n'
    else
        printf '  FAIL     the case destroyed a pre-existing occupant of the fixed spelling\n'
        failed=1
    fi
    [ "$gap_occupied_planted" -eq 1 ] && rm -rf "$gap_occupied"
fi

printf 'arch/deferral_record_test: predicate cases\n'

# The ledger the policy names. It has to be a tracked file in this
# repository, because the alternatives are both closed: the internal
# planning directory is git-ignored, and this repository has issues
# disabled, so "open an issue" is not a thing a contributor here can
# do. A policy that names an unreachable mechanism records nothing,
# which is the outcome section 14 exists to prevent.
expect_recoverable docs/deferred-work.md

# Tracked: the only state that actually survives a fresh clone.
# Names the suite runner rather than this file's own subject, which
# is not yet in the index while the invariant is being introduced —
# a bootstrap detail, not a property of the predicate.
expect_recoverable tests/arch/run-all.sh

# Ignored: the defect the invariant was written for.
expect_rejected .planning/follow-ups.md 'git-ignored'

# A name git would read as an option rather than a file. `--stage` is
# a real `ls-files` flag, so the query succeeds by listing the whole
# index and the absent record reads as present.
expect_rejected '--stage' 'an option name, not a tracked file'

# A name git would read as a glob. `[A]GENTS.md` matches the tracked
# `AGENTS.md` as a pathspec while the literal file does not exist, so
# the record is reported readable because something else is.
expect_rejected '[A]GENTS.md' 'a glob matching a tracked file, but not itself one'

# Absent: nothing ignores it, and it is still not there. This is the
# case an ignore-only check waves through.
expect_rejected docs/no-such-follow-ups.md 'not tracked'

# Present on disk but never added — same consequence as absent, since
# a clone reconstructs from the index, not from someone's disk.
#
# The probe must not be a fixed name. A developer with their own file
# at that path would have it truncated on the way in and deleted on
# the way out, so the suite would destroy unrelated local work as a
# side effect of asserting something unrelated to it.
first_probe=$(make_untracked_probe "$scratch_dir")
expect_rejected "$first_probe" 'untracked working-tree file'

# Non-collision, demonstrated without ever naming a path of our own.
# Writing a sentinel to a fixed location to prove the probe avoids
# fixed locations would reintroduce the hazard one line further down:
# the sentinel is someone's file too. So the first probe becomes the
# sentinel, and the second has to leave it alone.
printf 'do not lose me\n' >"$first_probe"
second_probe=$(make_untracked_probe "$scratch_dir")

if [ "$second_probe" = "$first_probe" ]; then
    printf '  FAIL     probe returned the same path twice\n'
    failed=1
elif [ "$(cat "$first_probe" 2>/dev/null)" != 'do not lose me' ]; then
    printf '  FAIL     probe clobbered an existing file at %s\n' "$first_probe"
    failed=1
else
    printf '  ok       probe never reuses or truncates a path\n'
fi

# Cleanup has to survive a repo cloned under a path with a space in
# it. An accumulator that joins paths with spaces cannot represent
# such a name, so the trap silently removes nothing and hands
# fragments to `rm -f` — litter left behind, and a wildcard away from
# removing something else.
spacey="$scratch_dir/a probe dir"
mkdir -p "$spacey"
spacey_probe=$(make_untracked_probe "$spacey")
# Run cleanup against the spacey directory in a subshell, so the
# assertion sees what the trap would actually do without ending the
# run's own scratch early.
(
    scratch_dir="$spacey"
    cleanup
)
if [ -e "$spacey_probe" ]; then
    printf '  FAIL     cleanup left a probe behind under a path with a space\n'
    failed=1
else
    printf '  ok       cleanup handles a path containing a space\n'
fi

# A cancelled run must stop, and must not report success. A handler
# that cleans up and returns leaves the script running against a
# directory it just deleted, and can still reach `exit 0` — so a
# Ctrl-C in CI looks like a pass.
probe_out=$(mktemp "$scratch_dir/signal-probe.XXXXXX")
# By absolute path. `$0` is whatever the caller typed, and this script
# has changed directory since — so launched as
# `sh ./deferral_record_test.sh` from `tests/arch`, the child cannot
# find itself, dies at once, and the case reports on a process that
# never received a signal.
ARCH_DEFERRAL_SIGNAL_PROBE=1 \
    sh "$REPO_ROOT/tests/arch/deferral_record_test.sh" >"$probe_out" 2>&1 &
probe_pid=$!
probe_record=''
i=0
while [ "$i" -lt 50 ] && [ -z "$probe_record" ]; do
    probe_record=$(head -n 1 "$probe_out" 2>/dev/null)
    [ -n "$probe_record" ] || sleep 0.1
    i=$((i + 1))
done
# The record crosses a pipe; the path must not. Serialized whole, a
# checkout under a directory whose name holds a newline prints one
# path as two records, the reader keeps the first fragment, and the
# suite fails despite a cleanup that worked. The record is a name
# whose spelling this file controls; the path is rebuilt where it is
# read. An empty record is judged by the three-outcome case below,
# not here.
case "$probe_record" in
    '') ;;
    */*)
        printf '  FAIL     the signal probe serialized a path, not a name: %s\n' "$probe_record"
        failed=1
        ;;
    *) printf '  ok       the signal probe reports a name, not a path\n' ;;
esac
# The child's scratch lives where this run's does; only the name
# travelled. An empty record must not reconstruct to the parent
# directory itself, which exists whether or not the probe ever ran.
probe_dir=''
[ -n "$probe_record" ] && probe_dir="$REPO_ROOT/tests/arch/$probe_record"
# Whether the directory was really there before the signal. Read after
# the kill, a path that never existed and a path that was cleaned up
# are the same observation, and the case cannot tell them apart.
probe_seen=0
[ -n "$probe_record" ] && [ -d "$probe_dir" ] && probe_seen=1

kill -TERM "$probe_pid" 2>/dev/null || true
# `set -e` would take the nonzero status of `wait` as a failure of
# this script, which is precisely the status being measured.
probe_status=0
wait "$probe_pid" || probe_status=$?

# Exactly 143, not merely nonzero. A child that dies of anything else —
# not finding itself, for instance — also exits nonzero, and the case
# then reports that cancellation works having measured a different
# failure entirely.
if [ "$probe_status" -ne 143 ]; then
    printf '  FAIL     a TERMed run exited %s, not 143 — it did not die of the signal\n' \
        "$probe_status"
    failed=1
else
    printf '  ok       a TERMed run exits on the signal (%s)\n' "$probe_status"
fi
# Three outcomes, not two. A child that printed nothing within the
# window leaves `probe_dir` empty, and treating that as "nothing left
# behind" reports that cleanup works having watched no directory at
# all — the same shape as every other vacuous pass on this branch.
if [ "$probe_seen" -eq 0 ]; then
    printf '  FAIL     the signal probe never showed a scratch directory to clean up\n'
    failed=1
elif [ -d "$probe_dir" ]; then
    printf '  FAIL     a TERMed run left its scratch directory behind\n'
    failed=1
    rm -rf "$probe_dir"
else
    printf '  ok       a TERMed run still cleans up\n'
fi

# The failure message has to name the actual reason. "Ignored" and
# "not tracked" call for different fixes — unignore it, or add it —
# and a check that reports the wrong one sends the reader the wrong
# way.
if [ "$(why_unrecoverable '.planning/follow-ups.md')" = 'which git ignores' ]; then
    printf '  ok       an ignored record is reported as ignored\n'
else
    printf '  FAIL     an ignored record is misreported as merely untracked\n'
    failed=1
fi
if [ "$(why_unrecoverable 'docs/no-such-follow-ups.md')" = 'which is not tracked by git' ]; then
    printf '  ok       an absent record is reported as untracked\n'
else
    printf '  FAIL     an absent record is misreported\n'
    failed=1
fi

# End-to-end over the check itself, against fixtures rather than the
# real contract file — mutating that to test it would risk leaving it
# mutated.
check_says() {
    fixture="$scratch_dir/agents-$2.md"
    printf '%s\n' "$1" >"$fixture"
    sh "$REPO_ROOT/tests/arch/deferral_record.sh" "$fixture" >/dev/null 2>&1
}

SECTION_HEAD='## 14. Scope is negotiable, delivery is not'

# A section with no declaration at all. The loop runs zero times and
# has nothing to report, so the check would print ok and pass — the
# invariant switched off by deleting one line, with no sign of it.
if check_says "$SECTION_HEAD

Some prose and no marker at all.
" nodecl; then
    printf '  FAIL     a section declaring no record passed — the invariant can be switched off silently\n'
    failed=1
else
    printf '  ok       a section declaring no record fails\n'
fi

# A malformed marker is the same hole by a different route: it parses
# to nothing rather than to a wrong path.
if check_says "$SECTION_HEAD

<!-- record path: tests/arch/run-all.sh -->
" malformed; then
    printf '  FAIL     a malformed marker passed as though nothing were declared\n'
    failed=1
else
    printf '  ok       a malformed marker fails\n'
fi

# A marker inside a fenced example is not a declaration. It would keep
# the at-least-one guard satisfied after the live marker was deleted,
# and the example's path is typically something tracked, so the check
# would pass having examined nothing that is actually the record.
#
# Same answer as CLAUDE.md's import: the section may not contain a
# fence at all. Scoped to the section, since AGENTS.md elsewhere uses
# fences legitimately and this rule has no business reaching them.
if check_says "$SECTION_HEAD

\`\`\`
<!-- record-path: AGENTS.md -->
\`\`\`
" fenced; then
    printf '  FAIL     a fenced example marker counted as a declaration\n'
    failed=1
else
    printf '  ok       a fenced example marker is not a declaration\n'
fi

# A heading that merely contains the phrase is a different section.
# Substring matching would adopt its body — so an appendix carrying a
# tracked marker could satisfy the guard after the real policy's
# marker was deleted, and the check would validate a destination the
# policy never declared.
if check_says "## Why Scope is negotiable, delivery is not failed

<!-- record-path: AGENTS.md -->
" lookalike; then
    printf '  FAIL     a heading merely containing the phrase was treated as the section\n'
    failed=1
else
    printf '  ok       only the exact heading is the section\n'
fi

# Two sections with the heading is ambiguous rather than twice as
# good: their bodies concatenate and one can cover for the other.
if check_says "$SECTION_HEAD

<!-- record-path: tests/arch/run-all.sh -->

$SECTION_HEAD

Some prose and no marker.
" duplicate; then
    printf '  FAIL     a duplicated section heading was accepted\n'
    failed=1
else
    printf '  ok       a duplicated section heading is refused\n'
fi

# A heading commented out alongside the live one is a second
# occurrence and fails on count — no HTML parsing required. The same
# answer as a duplicated heading, which is the point: uniqueness
# catches inert copies without anyone deciding what "inert" means.
if check_says "$SECTION_HEAD

<!-- record-path: tests/arch/run-all.sh -->

<!--
$SECTION_HEAD
-->
" commented_dup; then
    printf '  FAIL     a commented-out duplicate heading was tolerated\n'
    failed=1
else
    printf '  ok       a commented-out duplicate heading fails on count\n'
fi

# `git add -N` records only the intent to add. The entry is in the
# index with a regular mode and the empty blob, so every mode test
# passes — but `write-tree` omits the path, so the next commit and
# every clone from it lack the record entirely.
ita_repo="$scratch_dir/intent-to-add"
mkdir -p "$ita_repo"
(
    cd "$ita_repo"
    git init -q .
    printf 'log\n' >LEDGER.md
    git add -N LEDGER.md
) >/dev/null 2>&1
if (cd "$ita_repo" && record_is_recoverable LEDGER.md); then
    printf '  FAIL     an intent-to-add record was accepted\n'
    failed=1
else
    printf '  ok       an intent-to-add record is not carried\n'
fi

# Sourced scripts keep the caller's `$0`, so recomputing the root
# after the caller has already changed into it walks above the
# repository. Invoking by relative path from inside tests/arch is the

# An H2 indented one to three spaces is still an H2, so the body must
# stop there rather than swallowing the next section.
if check_says "$SECTION_HEAD

Prose, no marker.

  ## Next Section

<!-- record-path: tests/arch/run-all.sh -->
" indented_h2; then
    printf '  FAIL     the body ran past an indented H2 and adopted the next section\n'
    failed=1
else
    printf '  ok       an indented H2 ends the section\n'
fi

# The marker string may appear once in the whole file. An example
# anywhere — fenced, indented, quoted — is a second occurrence and
# fails, which takes the marker out of the fence question entirely.
if check_says "$SECTION_HEAD

<!-- record-path: tests/arch/run-all.sh -->

Later, an example: <!-- record-path: AGENTS.md -->
" two_markers; then
    printf '  FAIL     a second record-path mention was tolerated\n'
    failed=1
else
    printf '  ok       record-path may appear exactly once in the file\n'
fi

# And the check still passes what it should, so the guard above is not
# just rejecting everything.
if check_says "$SECTION_HEAD

<!-- record-path: tests/arch/run-all.sh -->
" good; then
    printf '  ok       a section declaring a tracked record passes\n'
else
    printf '  FAIL     a section declaring a tracked record was rejected\n'
    failed=1
fi

if check_says "$SECTION_HEAD

<!-- record-path: docs/nobody added this.md -->
" bad; then
    printf '  FAIL     a section declaring an untracked record passed\n'
    failed=1
else
    printf '  ok       a section declaring an untracked record fails\n'
fi

# An index entry is not the same as a readable record. A symlink and
# a gitlink both get one, and neither carries the content: the link
# may point outside the repository or at nothing, and a submodule is
# an empty directory until someone initialises it. Both would have the
# check report a durable record where a fresh clone finds none.
#
# Driven against a scratch repository, since the entries have to exist
# in an index and this one is not going to grow a symlink for the sake
# of a test. No commit is needed — `ls-files` reads the index.
linkrepo="$scratch_dir/linkrepo"
mkdir -p "$linkrepo"
(
    cd "$linkrepo"
    git init -q .
    printf 'content\n' >real.md
    ln -s /etc/hostname outside.md
    ln -s nowhere.md broken.md
    mkdir -p mixed allgood
    printf 'content\n' >mixed/content.md
    ln -s /etc/hostname mixed/outside.md
    printf 'content\n' >allgood/content.md
    git add real.md outside.md broken.md mixed allgood
) >/dev/null 2>&1

link_case() {
    if (cd "$linkrepo" && record_is_recoverable "$1"); then
        return 0
    fi
    return 1
}

if link_case real.md; then
    printf '  ok       a tracked regular file is a record\n'
else
    printf '  FAIL     a tracked regular file was rejected\n'
    failed=1
fi
# A declared directory is one record, so every entry under it has to
# carry content. One regular file is enough to satisfy a check that
# stops at the first match, and the clone still arrives with a link
# that points nowhere.
if link_case mixed/; then
    printf '  FAIL     a directory with one regular file and one symlink accepted\n'
    failed=1
else
    printf '  ok       a directory is rejected when any entry is a link\n'
fi
if link_case allgood/; then
    printf '  ok       a directory of regular files is a record\n'
else
    printf '  FAIL     a directory of regular files was rejected\n'
    failed=1
fi

for link in outside.md broken.md; do
    if link_case "$link"; then
        printf '  FAIL     tracked symlink %s accepted — the index entry is not the record\n' "$link"
        failed=1
    else
        printf '  ok       tracked symlink %s rejected\n' "$link"
    fi
done

# The reason has to match the fix. A tracked symlink is tracked, so
# telling its author it "is not tracked by git" sends them to `git
# add` a path git already has — the one action that cannot help.
if [ "$(cd "$linkrepo" && why_unrecoverable outside.md)" = 'which is not tracked by git' ]; then
    printf '  FAIL     a tracked symlink is reported as untracked — the suggested fix is impossible\n'
    failed=1
else
    printf '  ok       a tracked symlink is reported as the wrong kind of entry\n'
fi

# An import inside a fenced example is not an import. Claude Code does
# not evaluate `@path` in a code block, so moving the live line into
# one removes the contract from context — while a plain line-match
# still finds it and reports the invariant healthy.
fenced="$scratch_dir/claude-fenced.md"
printf 'Prose about the syntax:\n\n```\n@AGENTS.md\n```\n' >"$fenced"
if sh "$REPO_ROOT/tests/arch/agents_contract.sh" "$fenced" >/dev/null 2>&1; then
    printf '  FAIL     an import inside a fence passed — the contract would be out of context\n'
    failed=1
else
    printf '  ok       an import inside a fence does not count\n'
fi

# A fence closes only on the delimiter that opened it. A tilde block
# containing a backtick line, or a four-backtick block containing a
# three-backtick line, is one fenced region with an inner line of
# content — and a parser that toggles on any fence reads the inner one
# as the close and accepts the inert import after it.
nested_tilde="$scratch_dir/claude-nested-tilde.md"
printf '~~~\n```\n@AGENTS.md\n~~~\n' >"$nested_tilde"
if sh "$REPO_ROOT/tests/arch/agents_contract.sh" "$nested_tilde" >/dev/null 2>&1; then
    printf '  FAIL     a backtick line inside a tilde fence ended the fence\n'
    failed=1
else
    printf '  ok       a tilde fence is not closed by a backtick line\n'
fi

nested_ticks="$scratch_dir/claude-nested-ticks.md"
printf '````\n```\n@AGENTS.md\n````\n' >"$nested_ticks"
if sh "$REPO_ROOT/tests/arch/agents_contract.sh" "$nested_ticks" >/dev/null 2>&1; then
    printf '  FAIL     a shorter backtick run closed a longer fence\n'
    failed=1
else
    printf '  ok       a fence closes only on a run at least as long\n'
fi

# A fence with an info string does not close a region, and neither
# does one indented as a code block. Rather than teach the checker
# those rules — the third round of Markdown minutiae on a six-line
# pointer file — CLAUDE.md simply may not contain fences at all. With
# no fenced region anywhere, there is nowhere for an inert import to
# hide, and the question of where a region ends stops being asked.
infostring="$scratch_dir/claude-infostring.md"
printf '```\nexample\n``` text\n@AGENTS.md\n' >"$infostring"
if sh "$REPO_ROOT/tests/arch/agents_contract.sh" "$infostring" >/dev/null 2>&1; then
    printf '  FAIL     a run with an info string was treated as a close\n'
    failed=1
else
    printf '  ok       a fence with trailing text does not open the file up\n'
fi

indented_close="$scratch_dir/claude-indented.md"
printf '```\nexample\n    ```\n@AGENTS.md\n' >"$indented_close"
if sh "$REPO_ROOT/tests/arch/agents_contract.sh" "$indented_close" >/dev/null 2>&1; then
    printf '  FAIL     an indented run was treated as a close\n'
    failed=1
else
    printf '  ok       an indented run does not open the file up\n'
fi

# And a live import cannot be smuggled past by putting a fence
# elsewhere in the file, since fences are refused outright.
live_plus_fence="$scratch_dir/claude-live-plus-fence.md"
printf '@AGENTS.md\n\n```\nan example\n```\n' >"$live_plus_fence"
if sh "$REPO_ROOT/tests/arch/agents_contract.sh" "$live_plus_fence" >/dev/null 2>&1; then
    printf '  FAIL     a file containing any fence was accepted\n'
    failed=1
else
    printf '  ok       any fence at all is refused\n'
fi

# The import is only worth anything if what it names arrives with the
# clone. AGENTS.md sitting on disk proves nothing — `git rm --cached`
# leaves it there while removing it from the index, and a symlink to
# something outside the repository satisfies a file test too. Both
# leave a fresh clone importing a contract that is not there, while
# the reviewer's checkout looks fine.
untracked_contract="$scratch_dir/AGENTS-untracked.md"
printf 'the contract\n' >"$untracked_contract"
import_ok="$scratch_dir/claude-import-ok.md"
printf '@AGENTS.md\n' >"$import_ok"
if sh "$REPO_ROOT/tests/arch/agents_contract.sh" "$import_ok" "$untracked_contract" >/dev/null 2>&1; then
    printf '  FAIL     an untracked contract satisfied the import — a clone would have nothing to import\n'
    failed=1
else
    printf '  ok       the imported contract must be tracked, not merely present\n'
fi
# CLAUDE.md itself has to arrive with the clone. A tracked symlink to
# something generated or external satisfies a file test and reads
# fine here, while a fresh clone has no importing file at all.
claude_link_repo="$scratch_dir/claude-link"
mkdir -p "$claude_link_repo"
(
    cd "$claude_link_repo"
    git init -q .
    printf '@AGENTS.md\n' >elsewhere.md
    ln -s elsewhere.md CLAUDE.md
    printf 'contract\n' >AGENTS.md
    git add CLAUDE.md AGENTS.md elsewhere.md
) >/dev/null 2>&1
# Assert the reason, not only the exit status. An earlier version of
# this passed because the script could not find its own helpers under
# the overridden root — refused, but for a reason that had nothing to
# do with the symlink.
link_msg=$(ARCH_REPO_ROOT="$claude_link_repo" sh "$REPO_ROOT/tests/arch/agents_contract.sh" 2>&1 || true)
case "$link_msg" in
    *"tracked as a symlink or submodule"*)
        printf '  ok       CLAUDE.md must itself be a tracked regular file\n'
        ;;
    *)
        printf '  FAIL     symlinked CLAUDE.md not refused for being a symlink: %s\n' "$link_msg"
        failed=1
        ;;
esac

# A tracked directory of regular files satisfies the record predicate,
# which deliberately supports directory declarations — but `@AGENTS.md`
# has to name an importable file, not a folder.
dir_repo="$scratch_dir/agents-dir"
mkdir -p "$dir_repo/AGENTS.md"
(
    cd "$dir_repo"
    git init -q .
    printf '@AGENTS.md\n' >CLAUDE.md
    printf 'part\n' >AGENTS.md/part.md
    git add CLAUDE.md AGENTS.md
) >/dev/null 2>&1
if ARCH_REPO_ROOT="$dir_repo" sh "$REPO_ROOT/tests/arch/agents_contract.sh" >/dev/null 2>&1; then
    printf '  FAIL     AGENTS.md as a tracked directory was accepted\n'
    failed=1
else
    printf '  ok       the imported contract must be a file, not a directory\n'
fi

# The same shape with a regular tracked CLAUDE.md passes, so the guard
# is not simply rejecting every scratch repository.
plain_repo="$scratch_dir/claude-plain"
mkdir -p "$plain_repo"
(
    cd "$plain_repo"
    git init -q .
    printf '@AGENTS.md\n' >CLAUDE.md
    printf 'contract\n' >AGENTS.md
    git add CLAUDE.md AGENTS.md
) >/dev/null 2>&1
if ARCH_REPO_ROOT="$plain_repo" sh "$REPO_ROOT/tests/arch/agents_contract.sh" >/dev/null 2>&1; then
    printf '  ok       a regular tracked CLAUDE.md passes\n'
else
    printf '  FAIL     a regular tracked CLAUDE.md was rejected\n'
    failed=1
fi

if sh "$REPO_ROOT/tests/arch/agents_contract.sh" "$import_ok" AGENTS.md >/dev/null 2>&1; then
    printf '  ok       a tracked contract satisfies the import\n'
else
    printf '  FAIL     the real tracked contract was rejected\n'
    failed=1
fi

live="$scratch_dir/claude-live.md"
printf 'Prose.\n\n@AGENTS.md\n' >"$live"
if sh "$REPO_ROOT/tests/arch/agents_contract.sh" "$live" >/dev/null 2>&1; then
    printf '  ok       a real import passes\n'
else
    printf '  FAIL     a real import was rejected\n'
    failed=1
fi

printf 'arch/deferral_record_test: parser cases\n'

# The parser runs before the predicate, so anything it drops is never
# checked at all — a silent pass rather than a visible failure.

# A declared record: the marker line, exactly as the section writes it.
declared() { printf '<!-- record-path: %s -->\n' "$1" | cited_paths; }

# A mention in running prose, which is not a declaration.
mentioned() { printf 'text about `%s` in a sentence\n' "$1" | cited_paths; }

expect_parsed() {
    if [ "$(declared "$1")" = "$1" ]; then
        printf '  ok       parses %s\n' "$1"
    else
        printf '  FAIL     dropped %s — %s\n' "$1" "$2"
        failed=1
    fi
}

expect_not_parsed() {
    if [ -z "$(mentioned "$1")" ]; then
        printf '  ok       ignores %s (%s)\n' "$1" "$2"
    else
        printf '  FAIL     %s parsed as a path — %s\n' "$1" "$2"
        failed=1
    fi
}

expect_parsed 'docs/follow-ups.md' 'ordinary dotted file'

# Indentation makes a line code in Markdown at four spaces, and the
# boundary is not somewhere this check should have an opinion about —
# so a declaration must start at column zero, full stop. Anything
# indented is an example.
indented_marker() { printf '%s<!-- record-path: docs/x.md -->\n' "$1" | cited_paths; }
for pad in '    ' '  ' ' '; do
    if [ -z "$(indented_marker "$pad")" ]; then
        printf '  ok       an indented marker (%s spaces) is not a declaration\n' "${#pad}"
    else
        printf '  FAIL     an indented marker (%s spaces) counted as a declaration\n' "${#pad}"
        failed=1
    fi
done
expect_parsed 'FOLLOWUPS' 'git permits a name with neither slash nor dot'
expect_parsed 'docs/follow ups.md' 'git permits a space, so a citation must survive one'
expect_parsed '.planning/follow-ups' 'a record needs no extension to be a record'
expect_parsed 'docs/follow-ups/' 'a directory can be the record just as well'
expect_parsed '.planning/' 'named in this very section, and unreachable'

expect_parsed 'follow-ups.md' 'a record at the repo root is still a record'
expect_parsed './docs/follow-ups.md' 'the explicit repo-relative spelling'
expect_parsed './follow-ups.md' 'and the same at the root'
expect_parsed 'trailing-space.md ' 'git permits a name ending in a space'
expect_parsed "$(printf 'trailing-tab.md\t')" 'and one ending in a tab'
expect_parsed ' leading-space.md' 'and one beginning with a space'

expect_not_parsed 'piped' 'a bare word from the prose'
expect_not_parsed '#N · Title' 'a citation label, not a path'
expect_not_parsed 'git check-ignore' 'a command'
expect_not_parsed 'https://example.com/x' 'a URL'

if [ "$failed" -ne 0 ]; then
    printf 'arch/deferral_record_test: FAILED\n'
    exit 1
fi
printf 'arch/deferral_record_test: ok\n'
