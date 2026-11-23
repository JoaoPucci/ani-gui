#!/bin/sh
# Architectural invariant: both of the e2e launch helper's launches
# give the app the environment lib/e2e-launch-env.cjs builds.
#
# That helper drops a flags file the suite's shell left set
# (ANI_GUI_TEST_FLAGS_FILE), points every proxy variable at the
# refusing proxy and sets a test run's variables last; its own tests
# hold what it builds (electron/lib/e2e-launch-env.test.cjs). What they
# cannot see is whether a launch uses it: a launch that spread its
# environment by hand again would hand the app whatever the shell
# holds, and nothing would fail.
#
# A syntactic constraint, not interpretation: in electron/e2e/launch.ts,
# every `env:` written as an object key (indented three tabs or more,
# as an option inside a launch call is) reads `env: appEnv(`, and two
# do, one per launch (launchPackagedApp, spawnPackagedApp). It does not
# read which call a key sits in, and a launch added without an `env:`
# key, or with one written another way (`env,` shorthand, a variable),
# is not seen.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

LAUNCH='electron/e2e/launch.ts'
failed=0
tab="$(printf '\t')"

if [ ! -f "$LAUNCH" ]; then
    printf 'arch/e2e_launch_env FAIL: %s does not exist\n' "$LAUNCH" >&2
    exit 1
fi

by_hand=$(grep -nE "^${tab}${tab}${tab}+env:" "$LAUNCH" | grep -vE "env: appEnv\(" || true)
if [ -n "$by_hand" ]; then
    printf 'arch/e2e_launch_env FAIL: a launch in %s builds the environment by hand:\n%s\n' "$LAUNCH" "$by_hand" >&2
    printf 'build it with appEnv (electron/lib/e2e-launch-env.cjs)\n' >&2
    failed=1
fi

uses=$(grep -cE "^${tab}${tab}${tab}+env: appEnv\(" "$LAUNCH" || true)
if [ "$uses" != 2 ]; then
    printf 'arch/e2e_launch_env FAIL: %s has %s launches built with appEnv, expected 2\n' "$LAUNCH" "$uses" >&2
    failed=1
fi

exit "$failed"
