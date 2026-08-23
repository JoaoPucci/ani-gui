#!/usr/bin/env bats
#
# What the layering check can actually see in an import.
#
# The check is a `grep` over `use` lines, and the thing that keeps
# going wrong is not whether it runs but which spellings of the same
# import it recognises. `use crate::meta::Foo;` and
# `use crate::{meta::Foo, error::AniError};` are one dependency written
# two ways; a pattern that matches only the first passes a tree that
# violates the invariant, and reports ok while doing it.
#
# That gap was closed once by planting an import by hand and reading
# the output. It is committed here instead, because a violation planted
# in a terminal leaves nothing behind that can go red again — the next
# edit to the pattern is as free to narrow it as the last one was.
#
# Each case runs the real check against a repository it builds. The
# check derives its root from its own location (`dirname $0/../..`), so
# a copy placed at `<root>/tests/arch/` inspects `<root>` — no
# environment variable to set, and the program under test is the one
# the pull request ships rather than a re-implementation of it.

load '../helpers/loader'

setup() {
    CHECK="$REPO_ROOT/tests/arch/dep_direction.sh"
}

# A synthetic repository with the real check installed where it expects
# to find itself, and an empty backend tree for it to walk.
planted_repo() {
    root=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    mkdir -p "$root/tests/arch" "$root/backend/src"
    cp "$CHECK" "$root/tests/arch/dep_direction.sh"
    printf '%s\n' "$root"
}

# Write a Rust file under one of the backend's layer directories,
# creating the directory on the way. Body arrives on stdin so a case
# can spell the import exactly as rustfmt would leave it.
layer_file() {
    mkdir -p "$1/backend/src/$2"
    cat >"$1/backend/src/$2/$3"
}

@test "the repository as it stands satisfies every rule" {
    run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "an empty backend tree passes" {
    # The baseline the cases below plant into. If this were red, every
    # negative case would pass for the wrong reason.
    root=$(planted_repo)
    run sh "$root/tests/arch/dep_direction.sh"
    [ "$status" -eq 0 ]
}

@test "a plain layer import from scraper/ is caught" {
    root=$(planted_repo)
    layer_file "$root" scraper gate.rs <<'RS'
use crate::meta::kitsu::KitsuClient;
RS
    run sh "$root/tests/arch/dep_direction.sh"
    [ "$status" -ne 0 ]
    [[ "$output" == *'scraper/ may not import'* ]]
}

@test "a grouped layer import from scraper/ is caught" {
    # The idiomatic spelling, and the one the pattern used to walk
    # straight past: rustfmt merges sibling imports of the same crate
    # into a single brace group, so this is what the violation actually
    # looks like once anyone runs the formatter.
    root=$(planted_repo)
    layer_file "$root" scraper gate.rs <<'RS'
use crate::{error::AniError, meta::kitsu::KitsuClient};
RS
    run sh "$root/tests/arch/dep_direction.sh"
    [ "$status" -ne 0 ]
    [[ "$output" == *'scraper/ may not import'* ]]
}

@test "grouped imports naming commands or api are caught too" {
    # The rule names three layers. A pattern that grew a brace case for
    # one of them and not the others would satisfy the case above while
    # leaving two thirds of the invariant unenforced.
    for layer in commands api; do
        root=$(planted_repo)
        printf 'use crate::{%s::play::resolve, error::AniError};\n' "$layer" |
            layer_file "$root" scraper gate.rs
        run sh "$root/tests/arch/dep_direction.sh"
        [ "$status" -ne 0 ]
        [[ "$output" == *'scraper/ may not import'* ]]
    done
}

@test "a grouped import of modules scraper/ may use passes" {
    # The other half of the property. A pattern that matched any brace
    # group at all would make every case above pass and forbid the
    # imports the module is supposed to have.
    root=$(planted_repo)
    layer_file "$root" scraper gate.rs <<'RS'
use crate::{config::Paths, error::AniError};
use std::time::Duration;
RS
    run sh "$root/tests/arch/dep_direction.sh"
    [ "$status" -eq 0 ]
}

@test "the multi-line grouped form the header excludes is not caught" {
    # Pinning a documented gap rather than a behaviour worth having.
    # `dep_direction.sh` says in as many words that a brace group
    # rustfmt has split across lines is outside what a line-oriented
    # grep can see, and AGENTS.md §2 is what keeps a Rust parser out of
    # this suite. If someone closes the gap, this case goes red and the
    # header stops being true — which is the point: the claim and the
    # code fail together, instead of the claim quietly outliving it.
    root=$(planted_repo)
    layer_file "$root" scraper gate.rs <<'RS'
use crate::{
    error::AniError,
    meta::kitsu::KitsuClient,
};
RS
    run sh "$root/tests/arch/dep_direction.sh"
    [ "$status" -eq 0 ]
}

@test "cache/ importing reqwest is caught" {
    root=$(planted_repo)
    layer_file "$root" cache db.rs <<'RS'
use reqwest::Client;
RS
    run sh "$root/tests/arch/dep_direction.sh"
    [ "$status" -ne 0 ]
    [[ "$output" == *'cache/ may not import reqwest'* ]]
}

@test "proxy/ importing the metadata clients is caught" {
    root=$(planted_repo)
    layer_file "$root" proxy mod.rs <<'RS'
use crate::meta::anilist;
RS
    run sh "$root/tests/arch/dep_direction.sh"
    [ "$status" -ne 0 ]
    [[ "$output" == *'proxy/ may not import crate::meta'* ]]
}

@test "the frontend reaching outside its own src/ is caught" {
    root=$(planted_repo)
    mkdir -p "$root/frontend/src/lib"
    printf "import { thing } from '../../../backend/thing';\n" \
        >"$root/frontend/src/lib/thing.ts"
    run sh "$root/tests/arch/dep_direction.sh"
    [ "$status" -ne 0 ]
    [[ "$output" == *'may not relative-import outside'* ]]
}

@test "every violation in a tree is reported, not just the first" {
    # Four rules, one run. Stopping at the first would turn a layering
    # cleanup into one round per rule.
    root=$(planted_repo)
    layer_file "$root" scraper gate.rs <<'RS'
use crate::{meta::kitsu::KitsuClient, error::AniError};
RS
    layer_file "$root" cache db.rs <<'RS'
use reqwest::Client;
RS
    run sh "$root/tests/arch/dep_direction.sh"
    [ "$status" -ne 0 ]
    [[ "$output" == *'scraper/ may not import'* ]]
    [[ "$output" == *'cache/ may not import reqwest'* ]]
}
