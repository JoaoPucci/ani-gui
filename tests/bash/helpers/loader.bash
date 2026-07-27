# Common loader for every .bats file in tests/bash/.

# Require bats >= 1.5.0 so flags on `run` (e.g. --separate-stderr, -N) work.
bats_require_minimum_version 1.5.0

#
# Usage at the top of a .bats file:
#
#   load '../helpers/loader'
#
# This file is sourced (not executed) and:
#   1. Resolves repo paths into shell variables (REPO_ROOT, ANI_CLI_PATH, FIXTURES_DIR)
#   2. Loads bats-support, bats-assert, and bats-file from the vendored toolchain
#   3. Provides a single helper, source_ani_cli_lib, that sources ani-cli with
#      the __ANI_CLI_LIB__ guard set so the script does not auto-execute.
#
# It does NOT source ani-cli on its own — tests opt in. Acceptance tests that
# run the real CLI never source it.

# shellcheck shell=bash

# Resolve paths. BATS_TEST_DIRNAME points at the directory of the .bats file
# being run; we walk up until we find the repo root marker.
__loader_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$__loader_dir/../../.." && pwd)"
ANI_CLI_PATH="$REPO_ROOT/ani-cli"
FIXTURES_DIR="$REPO_ROOT/tests/fixtures"
BATS_VENDOR="$REPO_ROOT/tests/bash/.bats-vendor"

export REPO_ROOT ANI_CLI_PATH FIXTURES_DIR BATS_VENDOR

# Load bats helper libraries from the vendored checkouts.
# bats-support must be loaded before bats-assert.
# shellcheck source=/dev/null
load "$BATS_VENDOR/bats-support/load"
# shellcheck source=/dev/null
load "$BATS_VENDOR/bats-assert/load"
# shellcheck source=/dev/null
load "$BATS_VENDOR/bats-file/load"

# source_ani_cli_lib — source the CLI with the library-mode guard set so the
# main dispatcher at the bottom of the script does not run. After this
# returns, every function defined in ani-cli is callable directly from the
# test.
#
# Usage:
#   source_ani_cli_lib
#   run nth "select" <<<"$some_input"
#
# Bats runs tests with `set -e` (errexit). ani-cli has lines in its setup
# block such as `[ -t 0 ] || (command -v dmenu && use_external_menu=2)` that
# return non-zero on machines without dmenu/rofi installed — innocuous in
# normal CLI execution but fatal under errexit. We briefly disable errexit
# around the source and restore it afterward.
source_ani_cli_lib() {
    if [ ! -r "$ANI_CLI_PATH" ]; then
        printf 'ANI_CLI_PATH not readable: %s\n' "$ANI_CLI_PATH" >&2
        return 1
    fi
    # Relax errexit/errtrace and the bats ERR trap only AROUND the
    # source. ani-cli's setup block has innocuous non-zero lines (the
    # dmenu/rofi `command -v` checks) that would otherwise abort the
    # source mid-file, but both mechanisms are restored immediately
    # after — they are what makes a failing command or assertion later
    # in the test actually fail the test. Leaving them disabled for
    # the rest of the test (as this helper once did) turns every
    # post-source assertion into a silent no-op unless it happens to
    # be the last command.
    #
    # Tests that call ani-cli functions with intentional non-zero
    # exits (`grep -m 1` finding nothing, `nth` on empty stdin) use
    # `run`, which is immune to errexit, or explicit `|| capture`.
    local __err_trap
    __err_trap="$(trap -p ERR)"
    trap - ERR
    set +eE
    # shellcheck source=/dev/null
    __ANI_CLI_LIB__=1 . "$ANI_CLI_PATH" 2>/dev/null || true
    set -eE
    if [ -n "$__err_trap" ]; then
        eval "$__err_trap"
    fi
    return 0
}
