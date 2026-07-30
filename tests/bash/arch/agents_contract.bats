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
    printf '%s' "$1" > "$fixture"
    sh "$CHECK" "$fixture" >/dev/null 2>&1
}

@test "a live import passes" {
    contract_accepts '@AGENTS.md
'
}

@test "an import inside a fence does not count" {
    ! contract_accepts 'Prose about the syntax:

```
@AGENTS.md
```
'
}

@test "a tilde fence is not closed by a backtick line" {
    # One fenced region with an inner line of content. A parser that
    # toggles on any fence reads the inner line as the close and
    # accepts the inert import after it.
    ! contract_accepts '~~~
```
@AGENTS.md
~~~
'
}

@test "a fence closes only on a run at least as long" {
    ! contract_accepts '````
```
@AGENTS.md
````
'
}

@test "a fence with trailing text does not open the file up" {
    # An info string does not close a region.
    ! contract_accepts '```
example
``` text
@AGENTS.md
'
}

@test "an indented run does not open the file up" {
    ! contract_accepts '```
example
    ```
@AGENTS.md
'
}

@test "any fence at all is refused" {
    # A live import cannot be smuggled past by putting a fence
    # elsewhere in the file, because fences are refused outright.
    ! contract_accepts '@AGENTS.md

```
an example
```
'
}
