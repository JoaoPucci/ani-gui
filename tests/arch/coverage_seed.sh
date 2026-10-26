#!/bin/sh

# Architectural invariant: coverage runs generate the same property
# inputs every time.
#
# A file's coverage decides its CRAP score, and the CRAP gate fails a
# pull request when one more file crosses the high-risk bar. Property
# tests reach some lines only when a run happens to generate the input
# that does, so with a fresh seed each run a file near the bar crossed
# it or not by chance: one run of the gate went red on a file a local
# run had under it, for a line only a property's random suffix reached.
# The coverage runs therefore pin the seeds: the Rust one by running
# `cargo llvm-cov` only through tools/rust-lcov.sh, which exports
# PROPTEST_RNG_SEED; the frontend one through frontend/vite.config.ts,
# which hands the coverage run FAST_CHECK_SEED, and the setup file
# that gives it to fast-check. The ordinary test runs keep a fresh
# seed each time, which is where the properties earn their variety.
#
# The rules are syntactic:
#   - no workflow line names `cargo llvm-cov`: the runs go through the
#     script, and both coverage workflows name it;
#   - tools/rust-lcov.sh exports PROPTEST_RNG_SEED as a number, on a
#     line of its own at column zero;
#   - frontend/vite.config.ts names FAST_CHECK_SEED and the seed setup
#     file, and the setup file hands the seed to fast-check.
#
# What this does NOT check: that a step running the script is the one
# whose coverage the gate reads, or that a property's inputs depend on
# nothing but the seed (a HashMap's iteration order, the clock).
# Running the script is checked in tests/bash/arch/rust_lcov.bats.

set -eu

REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$REPO_ROOT"

failed=0
fail() {
    printf 'arch/coverage_seed FAIL: %s\n' "$1" >&2
    failed=1
}

if [ -d .github/workflows ]; then
    for wf in .github/workflows/*.yml .github/workflows/*.yaml; do
        [ -f "$wf" ] || continue
        if grep -n 'cargo llvm-cov' "$wf" >/dev/null; then
            fail "$wf runs cargo llvm-cov itself; run tools/rust-lcov.sh so the seed is pinned"
        fi
    done
    for wf in .github/workflows/crap.yml .github/workflows/rust.yml; do
        [ -f "$wf" ] || continue
        grep -q 'tools/rust-lcov\.sh' "$wf" || fail "$wf does not run tools/rust-lcov.sh"
    done
fi

if [ ! -f tools/rust-lcov.sh ]; then
    fail 'tools/rust-lcov.sh is missing'
elif ! grep -Eq '^export PROPTEST_RNG_SEED=[0-9]+$' tools/rust-lcov.sh; then
    fail 'tools/rust-lcov.sh does not export PROPTEST_RNG_SEED as a number'
fi

if [ -f frontend/vite.config.ts ]; then
    grep -q 'FAST_CHECK_SEED' frontend/vite.config.ts ||
        fail 'frontend/vite.config.ts does not hand the coverage run FAST_CHECK_SEED'
    grep -q 'tests/property-seed\.ts' frontend/vite.config.ts ||
        fail 'frontend/vite.config.ts does not load tests/property-seed.ts'
fi
if [ ! -f frontend/tests/property-seed.ts ]; then
    fail 'frontend/tests/property-seed.ts is missing'
elif ! grep -q 'configureGlobal' frontend/tests/property-seed.ts; then
    fail 'frontend/tests/property-seed.ts does not hand the seed to fast-check'
fi

if [ "$failed" -eq 0 ]; then
    printf 'arch/coverage_seed PASS\n'
fi
exit "$failed"
