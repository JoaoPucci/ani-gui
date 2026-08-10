#!/usr/bin/env bats
#
# CLAUDE.md must import AGENTS.md, and the import must be live.
#
# The reason this is checked at all: Claude Code loads CLAUDE.md
# automatically and follows an `@path` import, but it does not follow a
# prose pointer. An English "see AGENTS.md" left the working contract
# out of context entirely, which is how the rule it states went
# unapplied for months.
#
# Most cases below are about fences. A `@path` inside a code block is
# not evaluated, so moving the live line into one silently removes the
# contract — while a plain line match still finds it and reports the
# invariant healthy. Rather than teach a six-line pointer file the
# whole of Markdown's fence grammar, CLAUDE.md may contain no fence at
# all: with no fenced region anywhere there is nowhere for an inert
# import to hide, and where a region ends stops being a question.

load '../helpers/loader'

setup() {
    CHECK="$REPO_ROOT/tests/arch/agents_contract.sh"
}

# Write a candidate CLAUDE.md and run the check against it.
contract_accepts() {
    fixture="$BATS_TEST_TMPDIR/claude.md"
    printf '%s' "$1" >"$fixture"
    sh "$CHECK" "$fixture" >/dev/null 2>&1
}

# The cases below need an index of their own. Being on disk is not the
# property under test — arriving with a clone is — and this repository
# is not going to grow a symlinked CLAUDE.md or a directory called
# AGENTS.md for the sake of a test. The check takes its root by the
# namespaced variable precisely so a fixture can stand in for a
# checkout; no commit is needed, since `ls-files` reads the index.
new_repo() {
    mkdir -p "$1"
    (
        cd "$1" || exit 1
        git init -q .
    ) >/dev/null 2>&1
}

# Run the whole check against a scratch repository, capturing output —
# some cases assert the reason and not only the refusal.
check_repo() {
    ARCH_REPO_ROOT="$1" sh "$CHECK" 2>&1
}

@test "a live import passes" {
    contract_accepts '@AGENTS.md
'
}

@test "an import inside a fence does not count" {
    run ! contract_accepts 'Prose about the syntax:

```
@AGENTS.md
```
'
}

@test "a tilde fence is not closed by a backtick line" {
    # One fenced region with an inner line of content. A parser that
    # toggles on any fence reads the inner line as the close and
    # accepts the inert import after it.
    run ! contract_accepts '~~~
```
@AGENTS.md
~~~
'
}

@test "a fence closes only on a run at least as long" {
    run ! contract_accepts '````
```
@AGENTS.md
````
'
}

@test "a fence with trailing text does not open the file up" {
    # An info string does not close a region.
    run ! contract_accepts '```
example
``` text
@AGENTS.md
'
}

@test "an indented run does not open the file up" {
    run ! contract_accepts '```
example
    ```
@AGENTS.md
'
}

@test "any fence at all is refused" {
    # A live import cannot be smuggled past by putting a fence
    # elsewhere in the file, because fences are refused outright.
    run ! contract_accepts '@AGENTS.md

```
an example
```
'
}

# The fence cases above all vary a fixture's contents while the real
# tracked contract stands behind them. That leaves the other half of
# the check — whether what the import names actually arrives with a
# clone — resting on this repository being well-formed, which it is,
# so a regression there would pass in silence. The cases below drive
# the check against repositories built to be wrong.

@test "the imported contract must be tracked, not merely present" {
    # `git rm --cached` leaves AGENTS.md on disk while taking it out
    # of the index. The reviewer's checkout still reads fine; every
    # clone made from it imports a contract that is not there.
    repo="$BATS_TEST_TMPDIR/untracked-contract"
    new_repo "$repo"
    printf '@AGENTS.md\n' >"$repo/CLAUDE.md"
    printf 'the contract\n' >"$repo/AGENTS.md"
    (cd "$repo" && git add CLAUDE.md) >/dev/null 2>&1
    run ! check_repo "$repo"
}

@test "CLAUDE.md must itself be a tracked regular file" {
    # A file test follows a symlink, so a tracked link to something
    # generated or external reads fine while a fresh clone has no
    # importing file at all — the same defect as above, on the
    # importing side.
    repo="$BATS_TEST_TMPDIR/claude-link"
    new_repo "$repo"
    printf '@AGENTS.md\n' >"$repo/elsewhere.md"
    printf 'contract\n' >"$repo/AGENTS.md"
    ln -s elsewhere.md "$repo/CLAUDE.md"
    (cd "$repo" && git add CLAUDE.md AGENTS.md elsewhere.md) >/dev/null 2>&1

    # Assert the reason, not only the refusal. An earlier version of
    # this passed because the check could not find its own helpers
    # under the overridden root — refused, but for a reason with
    # nothing to do with the symlink.
    run check_repo "$repo"
    [ "$status" -ne 0 ]
    [[ "$output" == *"tracked as a symlink or submodule"* ]]
}

@test "the imported contract must be a file, not a directory" {
    # A tracked directory of regular files satisfies the record
    # predicate, which supports directory declarations on purpose. An
    # `@AGENTS.md` import has to name something readable, so the same
    # entry that makes a valid record makes an invalid contract.
    repo="$BATS_TEST_TMPDIR/agents-dir"
    new_repo "$repo"
    mkdir -p "$repo/AGENTS.md"
    printf '@AGENTS.md\n' >"$repo/CLAUDE.md"
    printf 'part\n' >"$repo/AGENTS.md/part.md"
    (cd "$repo" && git add CLAUDE.md AGENTS.md) >/dev/null 2>&1
    run ! check_repo "$repo"
}

@test "a regular tracked CLAUDE.md and contract pass" {
    # The same shape, built correctly. Without this the three cases
    # above are satisfied by a check that refuses every scratch
    # repository it is handed, for any reason at all.
    repo="$BATS_TEST_TMPDIR/well-formed"
    new_repo "$repo"
    printf '@AGENTS.md\n' >"$repo/CLAUDE.md"
    printf 'contract\n' >"$repo/AGENTS.md"
    (cd "$repo" && git add CLAUDE.md AGENTS.md) >/dev/null 2>&1
    run check_repo "$repo"
    [ "$status" -eq 0 ]
}
