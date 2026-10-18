#!/usr/bin/env bats
#
# What rust.yml's filter decides, by running it.
#
# The filter job's one step is tools/rust-workflow-filter.sh, which
# reads the paths a pull request changed and prints the three outputs
# the workflow's jobs gate on: `relevant`, the Rust pipeline, `picker`,
# the picker oracle, and `tracker_map`, the episode-map oracle. These cases build a scratch repository
# with a change of each shape and run the script over it, so what is
# asserted is the output the step would append to GITHUB_OUTPUT — not
# a reading of the script's text or of the workflow's.

load '../helpers/loader'

setup() {
    FILTER="$REPO_ROOT/tools/rust-workflow-filter.sh"
    repo=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    g init -q
    mkdir -p "$repo/backend/src/commands" "$repo/tests/picker-oracle" \
        "$repo/tests/tracker-map-oracle" \
        "$repo/docs" "$repo/frontend/src" "$repo/.github/workflows"
    for f in backend/src/commands/play_native_walk.rs \
        tests/picker-oracle/oracle.py tests/tracker-map-oracle/oracle.py \
        docs/title-resolution.md \
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

outputs() { printf 'relevant=%s\npicker=%s\ntracker_map=%s' "$1" "$2" "$3"; }

@test "a change under backend/ selects the Rust pipeline and both oracles" {
    head=$(change backend/src/commands/play_native_walk.rs)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs true true true)" ]
}

@test "the picker's model alone selects its oracle and not the Rust pipeline" {
    head=$(change tests/picker-oracle/oracle.py)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs false true false)" ]
}

@test "the picker golden alone selects both: the fixture test reads it" {
    head=$(change tests/picker-oracle/fixtures-golden.json)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs true true false)" ]
}

@test "the title-resolution docs alone select both oracles only" {
    head=$(change docs/title-resolution.md)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs false true true)" ]
}

@test "a recorded picker pool selects both" {
    head=$(change tests/fixtures/hianime/picker/11.json)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs true true false)" ]
}

@test "the toolchain pin selects the Rust pipeline only" {
    head=$(change rust-toolchain.toml)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs true false false)" ]
}

@test "the workflow itself selects everything" {
    head=$(change .github/workflows/rust.yml)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs true true true)" ]
}

@test "a frontend change alone selects neither" {
    head=$(change frontend/src/app.ts)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs false false false)" ]
}

@test "a picker file beside a frontend file selects the oracle" {
    head=$(change frontend/src/app.ts tests/picker-oracle/cases/doc-table.json)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs false true false)" ]
}

@test "a change touching no path selects neither" {
    g commit -q --allow-empty -m none
    head=$(g rev-parse HEAD)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs false false false)" ]
}

@test "a file renamed out of the oracle's tree still selects the oracle" {
    # The step lists a rename as both its paths, so a file leaving a
    # tree selects the jobs that read that tree.
    g mv tests/picker-oracle/oracle.py docs/oracle.py
    g commit -q -m rename
    head=$(g rev-parse HEAD)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs false true false)" ]
}

@test "any event but a pull request selects everything without reading a diff" {
    run filter push
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs true true true)" ]
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

@test "the episode-map model alone selects its oracle and not the Rust pipeline" {
    head=$(change tests/tracker-map-oracle/oracle.py)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs false false true)" ]
}

@test "the episode-map golden selects the Rust pipeline and its oracle" {
    head=$(change tests/tracker-map-oracle/fixtures-golden.json)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs true false true)" ]
}

@test "a recorded tracker case selects the Rust pipeline and the episode-map oracle" {
    head=$(change tests/fixtures/tracker-map/anilist.json)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs true false true)" ]
}

@test "a file renamed out of the episode-map oracle's tree still selects it" {
    g mv tests/tracker-map-oracle/oracle.py docs/tracker-oracle.py
    g commit -q -m rename
    head=$(g rev-parse HEAD)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "$(outputs false false true)" ]
}
