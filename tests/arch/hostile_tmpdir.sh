#!/bin/sh

# Architectural invariant: the backend tests run under a temporary
# directory whose name a shell, a TOML string, a printf format and a
# regular expression each read specially.
#
# Nearly every path the backend tests stage comes from the temporary
# directory, and many tests paste one into a stub script or a config
# file. Pasted
# unquoted, the path works under `/tmp` and breaks the test for anyone
# whose temporary directory has an apostrophe, a double quote or a
# backslash in its name. Fixing those tests one at a time did not
# hold: twice, a merge of master brought in new tests that pasted
# their paths as written, because nothing ran them under such a name.
# Reading a test's source cannot tell which of its strings become a
# script. Running the tests under the name can, and so CI does:
# rust.yml's Unix test legs point TMPDIR at the directory
# `tools/hostile-tmpdir.sh` creates. `tests/bash/arch/hostile_tmpdir.bats`
# asserts what that directory's name carries.
#
# What this file asserts is a syntactic constraint, per the AGENTS.md
# rule on how these checks may read source. Leading whitespace aside,
# rust.yml either has a line that is exactly the one running the tool
# and a line that is exactly the one handing its output on as TMPDIR,
# both above the line that runs `cargo test`, or it does not. A line
# commented out is not one of them.
#
# Deliberately not covered, and said so:
#   - whether those lines sit in the test job rather than another one,
#     and whether the condition the step runs under ever holds —
#     telling those apart means parsing YAML and the expression
#     language;
#   - a test that stages under some other root. The download cases in
#     `tests/backend_stop.rs`, the ones `start_a_download` builds, stage
#     their tree under CARGO_TARGET_TMPDIR, because the backend is
#     hard-linked in and a hard link needs one filesystem, so the
#     hostile name never reaches them. Neither stub they stage embeds a
#     path today: the yt-dlp stand-in finds its directory from `$0`,
#     and the transport needs none.
# A green run here means only that the workflow is configured to make
# the directory and export it before the tests run.

# The needles below are workflow text matched as fixed strings; the
# `$` in them is meant to stay unexpanded.
# shellcheck disable=SC2016

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

WORKFLOW=.github/workflows/rust.yml
failed=0

if [ ! -f tools/hostile-tmpdir.sh ]; then
    printf 'arch/hostile_tmpdir FAIL: tools/hostile-tmpdir.sh is missing\n' >&2
    failed=1
fi

# The line number of the first line of the workflow that is exactly
# `$1` once its indentation is set aside, or nothing.
line_of() {
    sed 's/^[[:space:]]*//' "$WORKFLOW" | grep -nxF -- "$1" | sed -n '1s/:.*//p'
}

test_line=$(line_of 'run: cargo test --workspace --all-features')
if [ -z "$test_line" ]; then
    printf 'arch/hostile_tmpdir FAIL: %s has no cargo test line to come before\n' "$WORKFLOW" >&2
    failed=1
fi

# The tool runs under the runner's own temporary root, and its output
# becomes TMPDIR for the steps after it.
for needle in \
    'dir=$(sh tools/hostile-tmpdir.sh "$RUNNER_TEMP")' \
    'printf '\''TMPDIR=%s\n'\'' "$dir" >> "$GITHUB_ENV"'; do
    at=$(line_of "$needle")
    if [ -z "$at" ]; then
        printf 'arch/hostile_tmpdir FAIL: %s has no line %s\n' "$WORKFLOW" "$needle" >&2
        failed=1
    elif [ -n "$test_line" ] && [ "$at" -gt "$test_line" ]; then
        printf 'arch/hostile_tmpdir FAIL: %s runs %s after cargo test\n' "$WORKFLOW" "$needle" >&2
        failed=1
    fi
done

if [ "$failed" -eq 0 ]; then
    printf 'arch/hostile_tmpdir: ok\n'
fi
exit "$failed"
