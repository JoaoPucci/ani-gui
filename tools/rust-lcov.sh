#!/bin/sh
# Generate the backend's lcov the way every coverage run does: in
# backend/, over the workspace, with proptest's RNG seeded the same way
# each time.
#
# Property tests reach some lines only when a run happens to generate
# the input that does, and a file's coverage decides its CRAP score,
# so with a fresh seed each run a file near the high-risk bar crossed
# it or not by chance. The seed is fixed here, for every coverage run,
# CI's and a local reproduction alike, and only here: the ordinary
# test runs keep a fresh seed each time, which is where the properties
# earn their variety. A caller's own PROPTEST_RNG_SEED is overridden,
# so a stray export cannot make two coverage runs disagree.
#
# Arguments after the script's own are passed to cargo llvm-cov.
#
#     sh tools/rust-lcov.sh            # writes backend/lcov.info

set -eu

export PROPTEST_RNG_SEED=20261026

cd "$(dirname "$0")/../backend"
exec cargo llvm-cov --workspace --lcov --output-path lcov.info "$@"
