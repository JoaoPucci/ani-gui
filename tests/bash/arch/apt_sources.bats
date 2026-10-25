#!/usr/bin/env bats
#
# What the apt-sources check can actually see.
#
# The check is a syntactic rule over workflow lines: wherever `apt` or
# `apt-get` is followed by an `update` word, the line must be the one
# refresh that reads only the distro's sources. These cases plant each
# spelling the rule is meant to refuse, and the spellings it must
# leave alone, in a repository built per case and reached through the
# ARCH_REPO_ROOT seam every check in tests/arch/ takes.

load '../helpers/loader'

CANON='sudo apt-get update -o Dir::Etc::sourcelist=/etc/apt/sources.list.d/ubuntu.sources -o Dir::Etc::sourceparts=-'

setup() {
    CHECK="$REPO_ROOT/tests/arch/apt_sources.sh"
}

planted_repo() {
    root=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    mkdir -p "$root/.github/workflows"
    printf '%s\n' "$root"
}

# A workflow whose one run step carries the given line.
plant() {
    printf 'jobs:\n  j:\n    steps:\n      - run: |\n          %s\n' "$2" \
        >"$1/.github/workflows/${3:-x.yml}"
}

@test "the real workflows pass" {
    run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "a tree with no workflows is a skip, not a pass about nothing" {
    root=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
    [[ "$output" == *skipping* ]]
}

@test "the distro-only refresh passes at any indentation" {
    root=$(planted_repo)
    plant "$root" "$CANON"
    printf '    %s\n' "$CANON" >>"$root/.github/workflows/x.yml"
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "a bare refresh is caught" {
    root=$(planted_repo)
    plant "$root" 'sudo apt-get update'
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *x.yml:5* ]]
}

@test "a refresh through apt rather than apt-get is caught" {
    root=$(planted_repo)
    plant "$root" 'sudo apt update'
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
}

@test "a refresh with flags before the verb is caught" {
    root=$(planted_repo)
    plant "$root" 'sudo apt-get -qq update'
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
}

@test "a tolerated distro-only refresh is caught" {
    # The exact command with its failure swallowed is not the command.
    root=$(planted_repo)
    plant "$root" "$CANON || true"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
}

@test "a refresh chained to the install on one line is caught" {
    root=$(planted_repo)
    plant "$root" 'sudo apt-get update && sudo apt-get install -y curl'
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
}

@test "a refresh ended by punctuation rather than a space is caught" {
    for line in 'sudo apt-get update; sudo apt-get install -y x' \
        'sudo apt-get update&&true' 'sudo apt-get update|| true' \
        'bash -c "apt-get update"' '(apt-get update)'; do
        root=$(planted_repo)
        plant "$root" "$line"
        ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    done
}

@test "the distro-only refresh as a one-line run key passes" {
    root=$(planted_repo)
    printf 'jobs:\n  j:\n    steps:\n      - run: %s\n      - name: x\n        run: %s\n' \
        "$CANON" "$CANON" >"$root/.github/workflows/x.yml"
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "a .yaml workflow is read too" {
    root=$(planted_repo)
    plant "$root" 'sudo apt-get update' x.yaml
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
}

@test "an install alone, or a package named update-something, is not a refresh" {
    root=$(planted_repo)
    plant "$root" 'sudo apt-get install -y update-notifier-common'
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
}
