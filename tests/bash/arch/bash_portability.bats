#!/usr/bin/env bats
#
# The divergence ceiling has to admit the patch set we mean to carry.
#
# `tests/arch/bash_portability.sh` counts how far the vendored `ani-cli`
# differs from upstream and fails past a ceiling, so an edit cannot creep
# in unrecorded. That only works if an unmodified checkout passes: a
# ceiling everything fails against reports the same thing whether or not
# a new patch landed, which is no report at all.

load '../helpers/loader'

setup() {
    ARCH_DIR="$REPO_ROOT/tests/arch"
    has_upstream() {
        git -C "$REPO_ROOT" remote get-url upstream >/dev/null 2>&1
    }
}

@test "the check passes against the committed script" {
    has_upstream || skip "no upstream remote to measure divergence against"
    run sh "$ARCH_DIR/bash_portability.sh"
    [ "$status" -eq 0 ]
}

@test "the check honours a root the caller resolved" {
    # Every other check in tests/arch/ takes ARCH_REPO_ROOT, which is
    # what lets a test point one at a scratch tree. Without it this
    # check can only ever measure the checkout it happens to live in,
    # so it cannot be driven from a copy — and a copy is how the case
    # below sabotages the ceiling.
    run env ARCH_REPO_ROOT="$BATS_TEST_TMPDIR" sh "$ARCH_DIR/bash_portability.sh"
    [[ "$output" != *"PASS"* ]]
}

@test "a ceiling nothing can satisfy still names the distance" {
    has_upstream || skip "no upstream remote to measure divergence against"
    # A ceiling of zero, so the check must fail. What matters is that
    # the failure says how far apart the two are: a reader can then
    # tell a newly landed patch from a ceiling that was never
    # satisfiable, which is the distinction the check exists to make.
    probe="$BATS_TEST_TMPDIR/portability.sh"
    sed 's/-gt 50 \]/-gt 0 ]/' "$ARCH_DIR/bash_portability.sh" >"$probe"
    run env ARCH_REPO_ROOT="$REPO_ROOT" sh "$probe"
    [ "$status" -ne 0 ]
    [[ "$output" == *"diverges from upstream"* ]]
}
