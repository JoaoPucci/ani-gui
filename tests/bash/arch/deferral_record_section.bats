#!/usr/bin/env bats
#
# The record check end to end, against fixture contract files.
#
# Every case here is a way the invariant could be switched off while
# still reporting a pass — a section with nothing declared, a marker
# that parses to nothing, a declaration inside a fenced example, a
# lookalike heading whose body gets adopted, a second copy of the
# heading whose body covers for the first. All of them leave the check
# printing ok having examined nothing that is actually the record.
#
# Fixtures rather than the real contract file: mutating that to test it
# risks leaving it mutated.

load '../helpers/loader'

SECTION_HEAD='## 14. Scope is negotiable, delivery is not'

setup() {
    CHECK="$REPO_ROOT/tests/arch/deferral_record.sh"
}

# Write a fixture contract and run the check against it.
check_accepts() {
    fixture="$BATS_TEST_TMPDIR/agents.md"
    printf '%s\n' "$1" > "$fixture"
    sh "$CHECK" "$fixture" >/dev/null 2>&1
}

@test "a section declaring no record fails" {
    # The loop runs zero times and has nothing to report, so an
    # unguarded check prints ok — the invariant switched off by
    # deleting one line, with no sign of it.
    ! check_accepts "$SECTION_HEAD

Some prose and no marker at all.
"
}

@test "a malformed marker fails" {
    # The same hole by another route: it parses to nothing rather than
    # to a wrong path.
    ! check_accepts "$SECTION_HEAD

<!-- record path: tests/arch/run-all.sh -->
"
}

@test "a marker inside a fenced example is not a declaration" {
    # It would keep the at-least-one guard satisfied after the live
    # marker was deleted, and an example's path is typically something
    # tracked — so the check passes having examined nothing real. The
    # answer is that the section may contain no fence at all, scoped to
    # the section since AGENTS.md uses fences legitimately elsewhere.
    ! check_accepts "$SECTION_HEAD

\`\`\`
<!-- record-path: AGENTS.md -->
\`\`\`
"
}

@test "a heading merely containing the phrase is a different section" {
    # Substring matching would adopt its body, so an appendix carrying
    # a tracked marker could satisfy the guard after the real policy's
    # marker was deleted — validating a destination the policy never
    # declared.
    ! check_accepts "## Why Scope is negotiable, delivery is not failed

<!-- record-path: AGENTS.md -->
"
}

@test "a duplicated section heading is refused" {
    # Two sections is ambiguous rather than twice as good: the bodies
    # concatenate and one can cover for the other.
    ! check_accepts "$SECTION_HEAD

<!-- record-path: tests/arch/run-all.sh -->

$SECTION_HEAD

Some prose and no marker.
"
}

@test "a commented-out duplicate heading fails on count" {
    # A second occurrence is a second occurrence, and no HTML parsing
    # is required to say so. That is the point of counting: uniqueness
    # catches inert copies without anyone having to define "inert".
    ! check_accepts "$SECTION_HEAD

<!-- record-path: tests/arch/run-all.sh -->

<!--
$SECTION_HEAD
-->
"
}

@test "a section declaring a tracked record passes" {
    check_accepts "$SECTION_HEAD

<!-- record-path: tests/arch/run-all.sh -->
"
}

@test "a section declaring an untracked record fails" {
    ! check_accepts "$SECTION_HEAD

<!-- record-path: docs/no-such-follow-ups.md -->
"
}

@test "an indented H2 ends the section" {
    # One to three spaces is still an H2, so the body must stop there
    # rather than swallowing the next section and adopting its marker.
    ! check_accepts "$SECTION_HEAD

Prose, no marker.

  ## Next Section

<!-- record-path: tests/arch/run-all.sh -->
"
}

@test "record-path may appear exactly once in the file" {
    # An example anywhere — fenced, indented, quoted — is a second
    # occurrence and fails, which takes the marker out of the fence
    # question entirely.
    ! check_accepts "$SECTION_HEAD

<!-- record-path: tests/arch/run-all.sh -->

Later, an example: <!-- record-path: AGENTS.md -->
"
}
