#!/bin/sh
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

__DEFERRAL_RECORD_LIB__=1
# shellcheck source=./deferral_record.sh
. "$REPO_ROOT/tests/arch/deferral_record.sh"
unset __DEFERRAL_RECORD_LIB__

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
scratch_dir=$(mktemp -d "$REPO_ROOT/tests/arch/.deferral-scratch.XXXXXX")
cleanup() { [ -n "${scratch_dir:-}" ] && rm -rf "$scratch_dir"; }

# EXIT owns cleanup; the signal handlers only have to end the run.
# Handling a signal without exiting returns control to the interrupted
# script, which then carries on against a directory it just deleted
# and can still reach `exit 0` — a cancelled CI job reporting a pass.
# The statuses are the conventional 128 + signal number.
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

# Probe mode: the run re-executes itself with this set so the signal
# case can watch a real process take a real signal, rather than
# inspecting trap definitions and hoping they mean what they say.
if [ -n "${DEFERRAL_SIGNAL_PROBE:-}" ]; then
    printf '%s\n' "$scratch_dir"
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
( scratch_dir="$spacey"; cleanup )
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
DEFERRAL_SIGNAL_PROBE=1 sh "$0" >"$probe_out" 2>&1 &
probe_pid=$!
probe_dir=''
i=0
while [ $i -lt 50 ] && [ -z "$probe_dir" ]; do
    probe_dir=$(head -n 1 "$probe_out" 2>/dev/null)
    [ -n "$probe_dir" ] || sleep 0.1
    i=$((i + 1))
done
kill -TERM "$probe_pid" 2>/dev/null || true
# `set -e` would take the nonzero status of `wait` as a failure of
# this script, which is precisely the status being measured.
probe_status=0
wait "$probe_pid" || probe_status=$?

if [ "$probe_status" -eq 0 ]; then
    printf '  FAIL     a TERMed run exited 0 — cancellation reads as success\n'
    failed=1
else
    printf '  ok       a TERMed run exits nonzero (%s)\n' "$probe_status"
fi
if [ -n "$probe_dir" ] && [ -d "$probe_dir" ]; then
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
    git add real.md outside.md broken.md
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
