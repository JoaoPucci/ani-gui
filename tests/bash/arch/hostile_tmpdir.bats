#!/usr/bin/env bats
#
# What tools/hostile-tmpdir.sh creates, by running it.
#
# rust.yml's Unix test legs point TMPDIR at the directory this tool
# makes. The paths the backend tests stage under the temporary
# directory then sit under a name that a shell, a TOML string, a printf
# format and a regular expression each read specially, so a test that
# pastes such a path into a stub script or a config file unquoted fails
# on the pull request that adds it. The
# run guards that only as far as the name carries those characters, so
# these cases assert the directory the tool creates, not the tool's
# text.

load '../helpers/loader'

setup() {
    TOOL="$REPO_ROOT/tools/hostile-tmpdir.sh"
    parent=$(mktemp -d "$BATS_TEST_TMPDIR/parent-XXXXXX")
}

@test "it creates a directory under the parent and prints its absolute path" {
    run sh "$TOOL" "$parent"
    [ "$status" -eq 0 ]
    [ -d "$output" ]
    [ "$(dirname "$output")" = "$parent" ]
    case "$output" in
        /*) ;;
        *) false ;;
    esac
}

@test "the name carries every character a stub script or config reads specially" {
    run sh "$TOOL" "$parent"
    [ "$status" -eq 0 ]
    name=$(basename "$output")
    # Each needle is matched as a fixed string: `\t` and `\n` are a
    # backslash and a letter, which is what dash's echo and a TOML
    # basic string read as an escape.
    for needle in "'" '"' '#' '$x' '`' '\t' '\n' '%s' '[' ']' '*' ';' '&' '|' ' '; do
        if ! printf '%s' "$name" | grep -qF -- "$needle"; then
            printf 'the name lacks %s: %s\n' "$needle" "$name" >&2
            false
        fi
    done
}

@test "the name carries no PATH separator and no newline" {
    run sh "$TOOL" "$parent"
    [ "$status" -eq 0 ]
    name=$(basename "$output")
    case "$name" in
        *:*) false ;;
    esac
    [ "${#lines[@]}" -eq 1 ]
}

@test "a second run over the same parent prints the same directory" {
    first=$(sh "$TOOL" "$parent")
    run sh "$TOOL" "$parent"
    [ "$status" -eq 0 ]
    [ "$output" = "$first" ]
    [ "$(find "$parent" -mindepth 1 -maxdepth 1 | wc -l)" -eq 1 ]
}

@test "a relative parent still yields an absolute path" {
    cd "$parent"
    mkdir rel
    run sh "$TOOL" rel
    [ "$status" -eq 0 ]
    [ "$(dirname "$output")" = "$parent/rel" ]
}

@test "an inherited CDPATH does not move the parent" {
    decoy=$(mktemp -d "$BATS_TEST_TMPDIR/decoy-XXXXXX")
    mkdir "$decoy/rel" "$parent/rel"
    cd "$parent"
    CDPATH="$decoy" run sh "$TOOL" rel
    [ "$status" -eq 0 ]
    [ "$(dirname "$output")" = "$parent/rel" ]
    [ -z "$(find "$decoy/rel" -mindepth 1)" ]
}

@test "a parent that is not a directory is refused and nothing is created" {
    run sh "$TOOL" "$parent/missing"
    [ "$status" -eq 1 ]
    [[ "$output" == *"is not a directory"* ]]
    [ ! -e "$parent/missing" ]
}

@test "no parent at all is refused" {
    run sh "$TOOL"
    [ "$status" -eq 2 ]
    [[ "$output" == *"usage: hostile-tmpdir.sh"* ]]
    run sh "$TOOL" ''
    [ "$status" -eq 2 ]
    [[ "$output" == *"usage: hostile-tmpdir.sh"* ]]
}
