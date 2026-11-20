#!/usr/bin/env bats
#
# Whether packages.yml builds the packages, by running the script its
# filter job runs.
#
# The workflow builds the Windows NSIS installer, the AppImage and the
# .deb. Started by hand it uploads them; on a pull request it only
# builds them, and its gate job is a required check. A Windows runner
# is slow, so a pull request builds only when it changes how a package
# is built or what it installs; any other event (a manual dispatch)
# always builds.
#
# These cases build a scratch repository with a change of each shape
# and run the script over it, so what is asserted is the output the
# step would append to GITHUB_OUTPUT.

load '../helpers/loader'

setup() {
    FILTER="$REPO_ROOT/tools/packages-filter.sh"
    repo=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    g init -q -b main
    mkdir -p "$repo/frontend/src" "$repo/electron"
    for f in frontend/src/app.ts electron/main.js electron/package.json; do
        printf 'before\n' >"$repo/$f"
        g add "$f"
    done
    g commit -q -m base
    base=$(g rev-parse HEAD)
}

g() {
    git -C "$repo" -c user.name=t -c user.email=t@example.invalid \
        -c commit.gpgsign=false "$@"
}

# Commit a change to the named paths and print the new head.
change() {
    for f in "$@"; do
        mkdir -p "$repo/$(dirname "$f")"
        printf 'after %s\n' "$RANDOM" >"$repo/$f"
        g add "$f"
    done
    g commit -q -m change
    g rev-parse HEAD
}

# The filter, run from the scratch repository as the step runs it from
# the checkout: event, base, head.
filter() {
    (cd "$repo" && sh "$FILTER" "$@")
}

@test "a manual dispatch builds without reading a diff" {
    run filter workflow_dispatch
    [ "$status" -eq 0 ]
    [ "$output" = "build=true" ]
}

@test "a change to app code alone does not build" {
    head=$(change frontend/src/app.ts electron/main.js backend/src/main.rs backend/Cargo.toml)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "build=false" ]
}

@test "a change touching no path does not build" {
    g commit -q --allow-empty -m none
    head=$(g rev-parse HEAD)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "build=false" ]
}

@test "each path that decides a package builds on its own" {
    # The packaging config and electron-builder's own version; the NSIS
    # script, the .deb's maintainer scripts and AppArmor profile, and
    # the afterPack hook; the fetchers of the bundled binaries and the
    # module they import; the module the afterPack hook requires on
    # every platform; the AppImage repack and the modules it requires;
    # the lockfile and workspace manifests that pin electron-builder;
    # the linker flags the backend is built with and the toolchain it
    # is built by; this workflow with its filter and the install
    # actions it calls. An electron-builder config file or an .npmrc
    # does not exist today, and either would be read by the build the
    # moment it did.
    for path in electron/package.json \
        electron/build-resources/installer.nsh \
        electron/build-resources/deb-after-install.sh \
        electron/build-resources/apparmor-profile \
        electron/build-resources/after-pack.cjs \
        electron/scripts/fetch-windows-deps.mjs \
        electron/scripts/fetch-linux-deps.mjs \
        electron/lib/dep-staging.cjs \
        electron/lib/linux-launcher.cjs \
        electron/scripts/repack-appimage.mjs \
        electron/lib/appimage-tree.cjs \
        electron/lib/sandbox-probe.cjs \
        pnpm-lock.yaml pnpm-workspace.yaml package.json \
        backend/.cargo/config.toml \
        rust-toolchain.toml \
        .github/workflows/packages.yml \
        tools/packages-filter.sh \
        .github/actions/pnpm-install/action.yml \
        .github/actions/apt-install/action.yml \
        electron/electron-builder.yml .npmrc electron/.npmrc; do
        head=$(change "$path")
        run filter pull_request "$base" "$head"
        [ "$status" -eq 0 ] || {
            echo "$path: exit $status"
            return 1
        }
        [ "$output" = "build=true" ] || {
            echo "$path: $output"
            return 1
        }
        base=$head
    done
}

@test "a name that only begins like a package path does not build" {
    # The patterns are anchored: a file beside the manifests, or one
    # whose name merely starts like a listed one, is not one of them.
    for path in electron/package.json.bak frontend/package.json \
        electron/scripts/fetch-linux-deps.mjs.orig \
        electron/lib/appimage-tree.test.cjs \
        backend/.cargo-notes.md \
        .github/workflows/packages.yml.disabled; do
        head=$(change "$path")
        run filter pull_request "$base" "$head"
        [ "$status" -eq 0 ]
        [ "$output" = "build=false" ] || {
            echo "$path: $output"
            return 1
        }
        base=$head
    done
}

@test "what the base branch changed since the branch point does not build" {
    # The event's base is the base branch's tip, which moves on as
    # master does. A lockfile bump that landed there after the branch
    # was cut is master's change, not this pull request's.
    feature=$(change frontend/src/app.ts)
    g checkout -q -b moved "$base"
    moved=$(change pnpm-lock.yaml)
    run filter pull_request "$moved" "$feature"
    [ "$status" -eq 0 ]
    [ "$output" = "build=false" ]
}

@test "a packaging change on a branch whose base moved still builds" {
    feature=$(change electron/package.json)
    g checkout -q -b moved "$base"
    moved=$(change frontend/src/app.ts)
    run filter pull_request "$moved" "$feature"
    [ "$status" -eq 0 ]
    [ "$output" = "build=true" ]
}

@test "a head that merged the base branch in is judged by its own changes" {
    # A pull request kept up to date merges master into its branch.
    # What master brought in is not the pull request's change.
    feature=$(change frontend/src/app.ts)
    g checkout -q -b moved "$base"
    moved=$(change pnpm-lock.yaml)
    g checkout -q "$feature"
    g merge -q --no-edit "$moved"
    head=$(g rev-parse HEAD)
    run filter pull_request "$moved" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "build=false" ]
}

@test "a file renamed out of a package path still builds" {
    g mv electron/package.json electron/package.old.json
    g commit -q -m rename
    head=$(g rev-parse HEAD)
    run filter pull_request "$base" "$head"
    [ "$status" -eq 0 ]
    [ "$output" = "build=true" ]
}

@test "a pull request without its two commits is refused" {
    run filter pull_request
    [ "$status" -ne 0 ]
    run filter pull_request "$base"
    [ "$status" -ne 0 ]
}

@test "a base the head shares no history with is refused, not read as no change" {
    # Without a merge base there is no diff to read. Silence would turn
    # into build=false and the required gate would pass unbuilt.
    g checkout -q --orphan other
    g rm -q -rf .
    other=$(change unrelated.txt)
    run filter pull_request "$other" "$base"
    [ "$status" -ne 0 ]
}

@test "no event at all is refused" {
    run filter
    [ "$status" -ne 0 ]
}
