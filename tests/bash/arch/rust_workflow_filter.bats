#!/usr/bin/env bats
#
# What rust.yml's filter decides, by running it.
#
# The filter job's one step is tools/rust-workflow-filter.sh, which
# reads the paths a pull request changed and prints the two outputs
# the workflow's jobs gate on: `relevant`, the Rust pipeline, and
# `picker`, the picker oracle. These cases build a scratch repository
# with a change of each shape and run the script over it, so what is
# asserted is the output the step would append to GITHUB_OUTPUT — not
# a reading of the script's text or of the workflow's.

load '../helpers/loader'

setup() {
    FILTER="$REPO_ROOT/tools/rust-workflow-filter.sh"
    repo=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    g init -q
    mkdir -p "$repo/backend/src/commands" "$repo/tests/picker-oracle" \
        "$repo/docs" "$repo/frontend/src" "$repo/.github/workflows"
    for f in backend/src/commands/play_native_walk.rs \
        tests/picker-oracle/oracle.py docs/title-resolution.md \
        frontend/src/app.ts .github/workflows/rust.yml; do
        printf 'before\n' >"$repo/$f"
        g add "$f"
    done
    g commit -q -m base
    base=$(g rev-parse HEAD)
}

# git in the scratch repository, with an identity of its own so the
# cases run the same wherever the suite does.
g() {
    git -C "$repo" -c user.name=t -c user.email=t@example.invalid \
        -c commit.gpgsign=false "$@"
}

# Commit a change to the named paths and print the new head.
change() {
    for f in "$@"; do
        mkdir -p "$repo/$(dirname "$f")"
        printf 'after\n' >"$repo/$f"
        g add "$f"
    done
    g commit -q -m change
    g rev-parse HEAD
}

# The filter, run from the scratch repository as the step runs it
# from the checkout.
filter() {
    (cd "$repo" && sh "$FILTER" "$@")
}

both() { printf 'relevant=%s\npicker=%s' "$1" "$2"; }

@test "a change under backend/ selects the Rust pipeline and the oracle" {
    head=$(change backend/src/commands/play_native_walk.rs)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both true true)" ]
}

@test "the model alone selects the oracle and not the Rust pipeline" {
    head=$(change tests/picker-oracle/oracle.py)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both false true)" ]
}

@test "the picker golden alone selects both: the fixture test reads it" {
    head=$(change tests/picker-oracle/fixtures-golden.json)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both true true)" ]
}

@test "the title-resolution docs alone select the oracle only" {
    head=$(change docs/title-resolution.md)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both false true)" ]
}

@test "a recorded picker pool selects both" {
    head=$(change tests/fixtures/hianime/picker/11.json)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both true true)" ]
}

@test "the toolchain pin selects the Rust pipeline only" {
    head=$(change rust-toolchain.toml)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both true false)" ]
}

@test "the hostile temporary-directory tool selects the Rust pipeline only" {
    head=$(change tools/hostile-tmpdir.sh)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both true false)" ]
}

@test "the workflow itself selects both" {
    head=$(change .github/workflows/rust.yml)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both true true)" ]
}

@test "a frontend change alone selects neither" {
    head=$(change frontend/src/app.ts)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both false false)" ]
}

@test "a picker file beside a frontend file selects the oracle" {
    head=$(change frontend/src/app.ts tests/picker-oracle/cases/doc-table.json)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both false true)" ]
}

@test "a change touching no path selects neither" {
    g commit -q --allow-empty -m none
    head=$(g rev-parse HEAD)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both false false)" ]
}

@test "a file renamed out of the oracle's tree still selects the oracle" {
    # The step lists a rename as both its paths, so a file leaving a
    # tree selects the jobs that read that tree.
    g mv tests/picker-oracle/oracle.py docs/oracle.py
    g commit -q -m rename
    head=$(g rev-parse HEAD)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(both false true)" ]
}

@test "any event but a pull request selects both without reading a diff" {
    run filter push
    [ "$status" -eq 0 ]
    [ "$output" = "$(both true true)" ]
}

@test "a pull request without its two commits is refused" {
    run filter pull_request
    [ "$status" -ne 0 ]
    run filter pull_request "$base"
    [ "$status" -ne 0 ]
}

@test "no event at all is refused" {
    run filter
    [ "$status" -ne 0 ]
}
