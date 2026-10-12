#!/usr/bin/env bats
#
# What the error-key check holds: every key the backend publishes has
# a message in every shipped locale, and no locale carries a message
# answering no key.
#
# The key list is the backend's own (`backend/error-keys.json`, held
# to the variants by `cargo test`); the locales are the ones
# `frontend/project.inlang/settings.json` ships; a key reaches the
# message named by the key with its dots as underscores. Each case
# runs the real check against a repository it builds, through the
# ARCH_REPO_ROOT seam every check in tests/arch/ takes.

load '../helpers/loader'

setup() {
    CHECK="$REPO_ROOT/tests/arch/error_keys.sh"
}

# A synthetic repository with two keys and two locales, consistent.
planted_repo() {
    root=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    mkdir -p "$root/backend" "$root/frontend/project.inlang"
    printf '["error.cache.generic", "error.network.upstream"]\n' \
        >"$root/backend/error-keys.json"
    printf '{"locales": ["en", "xx"]}\n' >"$root/frontend/project.inlang/settings.json"
    for locale in en xx; do
        mkdir -p "$root/frontend/messages/$locale"
        printf '{"error_cache_generic": "a", "error_network_upstream": "b {status}"}\n' \
            >"$root/frontend/messages/$locale/backend.json"
    done
    printf '%s\n' "$root"
}

@test "a consistent tree passes" {
    root=$(planted_repo)
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "this repository passes" {
    run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "a tree with no published key list is a skip, not a pass about nothing" {
    root=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
    [[ "$output" == *skipping* ]]
}

@test "a locale lacking a key's message is caught, and the key is named" {
    root=$(planted_repo)
    printf '{"error_cache_generic": "a"}\n' >"$root/frontend/messages/xx/backend.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *xx* ]]
    [[ "$output" == *error.network.upstream* ]]
}

@test "a message answering no key is caught" {
    root=$(planted_repo)
    printf '{"error_cache_generic": "a", "error_network_upstream": "b", "error_network_gone": "c"}\n' \
        >"$root/frontend/messages/en/backend.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *error_network_gone* ]]
}

@test "a message spelled as the key itself answers no key" {
    # The renderer reaches `error_network_upstream`, never a name with
    # dots in it; an entry spelled like the key is unreachable copy.
    root=$(planted_repo)
    printf '{"error_cache_generic": "a", "error.network.upstream": "b"}\n' \
        >"$root/frontend/messages/en/backend.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"no message for error.network.upstream"* ]]
    [[ "$output" == *"error.network.upstream\" answers no backend key"* ]]
}

@test "a shipped locale with no catalogue at all is caught" {
    root=$(planted_repo)
    rm "$root/frontend/messages/xx/backend.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *xx* ]]
}

@test "an empty message is caught" {
    # The compile does not fall back for an entry that exists and says
    # nothing; the screen would show nothing for that failure.
    root=$(planted_repo)
    printf '{"error_cache_generic": "", "error_network_upstream": "b"}\n' \
        >"$root/frontend/messages/xx/backend.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *error.cache.generic* ]]
}

@test "a catalogue that is not JSON is caught rather than crashed on" {
    root=$(planted_repo)
    printf 'not json\n' >"$root/frontend/messages/en/backend.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *en/backend.json* ]]
}

@test "a key list that is not JSON is reported, not crashed on" {
    root=$(planted_repo)
    printf 'not json\n' >"$root/backend/error-keys.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"arch/error_keys FAIL"* ]]
    [[ "$output" == *backend/error-keys.json* ]]
}

@test "a missing or empty locale list is reported, not crashed on" {
    root=$(planted_repo)
    rm "$root/frontend/project.inlang/settings.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"arch/error_keys FAIL"* ]]
    printf 'null\n' >"$root/frontend/project.inlang/settings.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"arch/error_keys FAIL"* ]]
}

@test "a catalogue that parses to something other than an object is reported" {
    root=$(planted_repo)
    printf 'null\n' >"$root/frontend/messages/en/backend.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"arch/error_keys FAIL"* ]]
    [[ "$output" == *en/backend.json* ]]
}

@test "a key listed twice is caught" {
    root=$(planted_repo)
    printf '["error.cache.generic", "error.cache.generic", "error.network.upstream"]\n' \
        >"$root/backend/error-keys.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"error.cache.generic"* ]]
}

@test "two keys that rewrite to one message name are caught" {
    # `error.a_b.c` and `error.a.b_c` both reach `error_a_b_c`; one
    # message would answer both and say the right thing for at most one.
    root=$(planted_repo)
    printf '["error.a_b.c", "error.a.b_c"]\n' >"$root/backend/error-keys.json"
    for locale in en xx; do
        printf '{"error_a_b_c": "a"}\n' >"$root/frontend/messages/$locale/backend.json"
    done
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *error_a_b_c* ]]
}
