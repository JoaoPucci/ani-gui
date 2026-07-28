#!/bin/sh
# Arch-layer wrapper around the node:test suites for the scripts in
# tools/. Those scripts are plain Node, so their tests stay outside
# the Vitest/cargo trees — this shim just exposes them to the arch
# runner (and thus to arch.yml).
#
# Runs the whole directory rather than naming files, so a new
# tests/tools/*.test.mjs is covered the moment it is written.

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$REPO_ROOT"
node --test tests/tools/*.test.mjs
