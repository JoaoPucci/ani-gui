#!/usr/bin/env bats
#
# What the reacquisition check can actually see.
#
# The check is greps over routes that could reacquire the retired
# script — sourcing, the library seam, manifest staging, workflow
# installs, fetcher declarations — and what has kept going wrong is
# which spellings of a route the patterns recognise: a template
# literal is a declaration as much as a quoted string, and
# `install ./ani-cli` is an install as much as `install ani-cli`.
# Both gaps were found by planting the violation in a terminal, and a
# violation planted in a terminal leaves nothing behind that can go
# red again. These cases commit the plantings.
#
# Each case runs the real check against a repository it builds,
# through the ARCH_REPO_ROOT seam every check in tests/arch/ takes.

load '../helpers/loader'

setup() {
    CHECK="$REPO_ROOT/tests/arch/no_reacquired_script.sh"
}

# A synthetic repository with the directories the check walks.
planted_repo() {
    root=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    mkdir -p "$root/backend" "$root/frontend" "$root/electron/scripts" \
        "$root/.github/workflows"
    printf '%s\n' "$root"
}

@test "a clean tree passes" {
    root=$(planted_repo)
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "a tree with no app directories is a skip, not a pass about nothing" {
    root=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
    [[ "$output" == *skipping* ]]
}

@test "sourcing the script inside the app is caught" {
    root=$(planted_repo)
    printf 'source ./ani-cli\n' >"$root/frontend/launch.sh"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
}

@test "reviving the library seam inside the app is caught" {
    root=$(planted_repo)
    printf 'let guard = "__ANI_CLI_LIB__";\n' >"$root/backend/seam.rs"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
}

@test "a manifest staging the script is caught" {
    root=$(planted_repo)
    printf '{"extraResources": [{"from": "vendor/ani-cli"}]}\n' \
        >"$root/electron/package.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
}

@test "a workflow installing the script is caught, bare or path-prefixed" {
    for operand in 'ani-cli' './ani-cli' 'vendor/ani-cli'; do
        root=$(planted_repo)
        printf 'run: sudo install -m 0755 %s /usr/local/bin/ani-cli\n' \
            "$operand" >"$root/.github/workflows/x.yml"
        ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    done
}

@test "a fetcher declaration is caught in every string syntax" {
    # A name field, a URL, and an output path — one per quote syntax,
    # including the template literal the pattern once missed.
    root=$(planted_repo)
    printf "export const DEPS = [{ name: 'ani-cli' }];\n" \
        >"$root/electron/scripts/fetch-linux-deps.mjs"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"

    root=$(planted_repo)
    printf 'const u = "https://host/x/ani-cli";\n' \
        >"$root/electron/scripts/fetch-linux-deps.mjs"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"

    root=$(planted_repo)
    # shellcheck disable=SC2016
    printf 'const u = `https://host/${version}/ani-cli`;\n' \
        >"$root/electron/scripts/fetch-windows-deps.mjs"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
}

@test "a bare prose mention in a fetcher comment is not a declaration" {
    root=$(planted_repo)
    printf '// the transport ani-cli 5.0 prefers\nexport const DEPS = [];\n' \
        >"$root/electron/scripts/fetch-windows-deps.mjs"
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
}
