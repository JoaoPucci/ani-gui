# Common loader for every .bats file in tests/bash/ — today that is
# the arch harness under tests/bash/arch/, whose subjects are the
# checks in tests/arch/ and the deferred-work log's structure.

# Require bats >= 1.5.0 so flags on `run` (e.g. --separate-stderr, -N) work.
bats_require_minimum_version 1.5.0

#
# Usage at the top of a .bats file:
#
#   load '../helpers/loader'
#
# This file is sourced (not executed) and:
#   1. Resolves repo paths into shell variables (REPO_ROOT, FIXTURES_DIR)
#   2. Loads bats-support, bats-assert, and bats-file from the vendored toolchain

# shellcheck shell=bash

# Resolve paths. BATS_TEST_DIRNAME points at the directory of the .bats file
# being run; we walk up until we find the repo root marker.
__loader_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$__loader_dir/../../.." && pwd)"
FIXTURES_DIR="$REPO_ROOT/tests/fixtures"
BATS_VENDOR="$REPO_ROOT/tests/bash/.bats-vendor"

export REPO_ROOT FIXTURES_DIR BATS_VENDOR

# Load bats helper libraries from the vendored checkouts.
# bats-support must be loaded before bats-assert.
# shellcheck source=/dev/null
load "$BATS_VENDOR/bats-support/load"
# shellcheck source=/dev/null
load "$BATS_VENDOR/bats-assert/load"
# shellcheck source=/dev/null
load "$BATS_VENDOR/bats-file/load"
