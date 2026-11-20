#!/bin/sh
# Whether packages.yml builds the packages — the Windows installer, the
# AppImage and the .deb — as the one output its filter job sets. The
# filter step is one call,
#
#     tools/packages-filter.sh <event-name> <base-sha> <head-sha>
#
# and this prints `build=true|false` for the step to append to
# GITHUB_OUTPUT. On any event but a pull request — a manual dispatch —
# it is true and no diff is read. On a pull request the paths the pull
# request itself changed decide: the diff runs from where the head
# branched off the base (`base...head`), so what the base branch gained
# since, whether or not the head has merged it in, is not counted. A
# rename is listed as both its paths, so a file leaving one of the
# paths below still builds. A head that shares no history with the base
# has no such point, and is refused rather than read as no change: the
# workflow's required gate passes on build=false.
#
# The diff is captured first and grep reads the captured text, for the
# reason tools/rust-workflow-filter.sh gives: `git diff | grep -q`
# under pipefail reads an early grep exit as a failed diff.
#
# tests/bash/arch/packages_filter.bats runs this over scratch
# repositories holding a change of each shape it has to tell apart.

set -eu

event="${1:?event name}"

if [ "$event" != "pull_request" ]; then
    echo 'build=true'
    exit 0
fi

base="${2:?base sha}"
head="${3:?head sha}"
changed=$(git diff --name-only --no-renames "$base...$head")

# What decides how a package is built or what it installs, beyond the
# app's own code: electron-builder's config and version (the electron
# manifest, a config file of its own should one appear, the lockfile and
# the workspace manifests that pin it, and an .npmrc pnpm would read);
# the NSIS script, the .deb's maintainer scripts and AppArmor profile
# and the afterPack hook, all under build-resources; the fetchers of the
# bundled binaries and the module they import; the module the afterPack
# hook requires on every platform; the AppImage repack and the modules
# it requires; the linker flags the backend is built with and the
# toolchain that builds it; and this workflow with its filter and the
# install actions it calls. A change to the app's code changes what a
# package carries, not how it is built, and the other workflows build
# and test that code on every platform.
if printf '%s\n' "$changed" |
    grep -qE '^(electron/package\.json$|electron/electron-builder\.|(electron/)?\.npmrc$|electron/build-resources/|electron/scripts/fetch-(windows|linux)-deps\.mjs$|electron/scripts/repack-appimage\.mjs$|electron/lib/(dep-staging|linux-launcher|appimage-tree|sandbox-probe)\.cjs$|pnpm-lock\.yaml$|pnpm-workspace\.yaml$|package\.json$|backend/\.cargo/|rust-toolchain\.toml$|tools/packages-filter\.sh$|\.github/workflows/packages\.yml$|\.github/actions/(pnpm|apt)-install/)'; then
    echo 'build=true'
else
    echo 'build=false'
fi
