#!/bin/sh
# Which of rust.yml's jobs a change concerns, as the three outputs its
# filter job sets: `relevant`, the Rust pipeline, `picker`, the picker
# oracle, and `tracker_map`, the episode-map oracle. The filter step is
# one call,
#
#     tools/rust-workflow-filter.sh <event-name> <base-sha> <head-sha>
#
# and this prints `relevant=true|false`, `picker=true|false` and
# `tracker_map=true|false`, one per line, for the step to append to
# GITHUB_OUTPUT. On any event but a pull request — a push to master —
# all three are true and no diff is read. On a pull request the paths changed between the two commits
# decide; a rename is listed as both its paths, so a file leaving a
# tree still selects the jobs that read that tree.
#
# The diff is captured first and grep reads the captured text. The
# naive `git diff | grep -q` pipeline interacts badly with pipefail:
# grep -q closes the pipe on its first match, git diff dies of SIGPIPE,
# the pipeline exits non-zero, and a relevant change reads as none.
#
# tests/bash/arch/rust_workflow_filter.bats runs this over scratch
# repositories holding a change of each shape it has to tell apart, so
# what a path selects is executed, never read off this file.

set -eu

event="${1:?event name}"

if [ "$event" != "pull_request" ]; then
    printf 'relevant=true\npicker=true\ntracker_map=true\n'
    exit 0
fi

base="${2:?base sha}"
head="${3:?head sha}"
changed=$(git diff --name-only --no-renames "$base" "$head")

# The Rust pipeline: the sources, every fixture the tests read, the
# toolchain pin, the two goldens the fixture tests compare against,
# and the workflow itself.
if printf '%s\n' "$changed" |
    grep -qE '^(backend/|tests/fixtures/|rust-toolchain\.toml$|tests/(picker|tracker-map)-oracle/fixtures-golden\.json$|\.github/workflows/rust\.yml$)'; then
    echo 'relevant=true'
else
    echo 'relevant=false'
fi

# The picker oracle reads the picker, its recorded pools, its own model
# and cases, the document the model transcribes, and the workflow; a
# change to the document alone is no reason to run the rest.
if printf '%s\n' "$changed" |
    grep -qE '^(backend/|tests/fixtures/hianime/picker/|tests/picker-oracle/|docs/title-resolution\.md$|\.github/workflows/rust\.yml$)'; then
    echo 'picker=true'
else
    echo 'picker=false'
fi

# The episode-map oracle reads the rules and their evidence readers,
# the recorded tracker cases, its own model and allowlist, the
# document the model transcribes, and the workflow.
if printf '%s\n' "$changed" |
    grep -qE '^(backend/|tests/fixtures/tracker-map/|tests/tracker-map-oracle/|docs/title-resolution\.md$|\.github/workflows/rust\.yml$)'; then
    echo 'tracker_map=true'
else
    echo 'tracker_map=false'
fi
