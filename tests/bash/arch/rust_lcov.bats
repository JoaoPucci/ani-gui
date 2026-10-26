#!/usr/bin/env bats
#
# What tools/rust-lcov.sh runs, by running it.
#
# The coverage workflows produce the Rust lcov through this script so
# that property tests generate the same inputs on every coverage run
# (see tests/arch/coverage_seed.sh). These cases put a stand-in cargo
# first on PATH that records what it was given, and run the script.

load '../helpers/loader'

setup() {
    SCRIPT="$REPO_ROOT/tools/rust-lcov.sh"
    bin=$(mktemp -d "$BATS_TEST_TMPDIR/bin-XXXXXX")
    out="$BATS_TEST_TMPDIR/cargo.out"
    cat >"$bin/cargo" <<STUB
#!/bin/sh
{
    printf 'seed=%s\n' "\${PROPTEST_RNG_SEED-unset}"
    printf 'dir=%s\n' "\$(basename "\$PWD")"
    printf 'args=%s\n' "\$*"
} >"$out"
STUB
    chmod +x "$bin/cargo"
}

@test "it runs llvm-cov in backend/ with a fixed proptest seed" {
    PATH="$bin:$PATH" run sh "$SCRIPT"
    assert_success
    run cat "$out"
    assert_line --regexp '^seed=[0-9]+$'
    assert_line 'dir=backend'
    assert_line 'args=llvm-cov --workspace --lcov --output-path lcov.info'
}

@test "the seed is the same on every run, whatever the caller exported" {
    PATH="$bin:$PATH" run sh "$SCRIPT"
    first=$(grep '^seed=' "$out")
    PROPTEST_RNG_SEED=12345 PATH="$bin:$PATH" run sh "$SCRIPT"
    assert_success
    second=$(grep '^seed=' "$out")
    [ "$first" = "$second" ]
}

@test "arguments after the script's own reach cargo" {
    PATH="$bin:$PATH" run sh "$SCRIPT" --no-clean
    assert_success
    run cat "$out"
    assert_line 'args=llvm-cov --workspace --lcov --output-path lcov.info --no-clean'
}
