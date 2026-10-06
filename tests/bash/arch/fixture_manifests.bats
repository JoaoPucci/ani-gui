#!/usr/bin/env bats
#
# What the fixture-manifest check accepts and refuses.
#
# The check holds each MANIFEST.json under tests/fixtures/ to the files
# beside it. Its own rules — which bytes a digest covers, which names an
# entry may use — are decided in the check, so they are driven here
# against trees the cases build, through the ARCH_REPO_ROOT seam every
# check in tests/arch/ takes.

load '../helpers/loader'

setup() {
    CHECK="$REPO_ROOT/tests/arch/fixture_manifests.sh"
    root=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    mkdir -p "$root/tests/fixtures/set" "$root/tests/fixtures/other"
    printf 'hello\n' >"$root/tests/fixtures/set/a.txt"
    printf 'elsewhere\n' >"$root/tests/fixtures/other/x.txt"
    printf 'nested\n' >"$root/tests/fixtures/set/x.txt"
}

sha() { sha256sum <"$1" | cut -d' ' -f1; }

# Write tests/fixtures/set/MANIFEST.json with one entry per
# name=path pair, each recording the sha256 and size of `path`.
manifest() {
    {
        printf '{"fixtures": {'
        sep=''
        for pair in "$@"; do
            name=${pair%%=*}
            file=${pair#*=}
            printf '%s"%s": {"sha256": "%s", "size": %s}' \
                "$sep" "$name" "$(sha "$file")" "$(wc -c <"$file" | tr -d ' ')"
            sep=', '
        done
        printf '}}\n'
    } >"$root/tests/fixtures/set/MANIFEST.json"
}

@test "a manifest that matches its directory passes" {
    rm "$root/tests/fixtures/set/x.txt"
    manifest "a.txt=$root/tests/fixtures/set/a.txt"
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "a fixture whose bytes changed under its digest is caught" {
    rm "$root/tests/fixtures/set/x.txt"
    manifest "a.txt=$root/tests/fixtures/set/a.txt"
    printf 'changed\n' >"$root/tests/fixtures/set/a.txt"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"a.txt: sha256 is"* ]]
}

@test "a file beside a manifest but not listed in it is caught" {
    manifest "a.txt=$root/tests/fixtures/set/a.txt"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"x.txt sits beside the manifest but is not listed"* ]]
}

@test "a base64 entry is checked over its decoded bytes" {
    rm "$root/tests/fixtures/set/x.txt"
    base64 "$root/tests/fixtures/set/a.txt" >"$root/tests/fixtures/set/a.b64"
    rm "$root/tests/fixtures/set/a.txt"
    printf '{"fixtures": {"a.b64": {"encoding": "base64", "sha256": "%s", "size": 6}}}\n' \
        "$(printf 'hello\n' | sha256sum | cut -d' ' -f1)" \
        >"$root/tests/fixtures/set/MANIFEST.json"
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "an entry naming a file above its directory is refused" {
    manifest "a.txt=$root/tests/fixtures/set/a.txt" \
        "x.txt=$root/tests/fixtures/set/x.txt" \
        "../other/x.txt=$root/tests/fixtures/other/x.txt"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"../other/x.txt: entry name is not a file beside the manifest"* ]]
}

@test "an entry naming a file in a subdirectory is refused" {
    mkdir "$root/tests/fixtures/set/sub"
    printf 'deeper\n' >"$root/tests/fixtures/set/sub/y.txt"
    manifest "a.txt=$root/tests/fixtures/set/a.txt" \
        "x.txt=$root/tests/fixtures/set/x.txt" \
        "sub/y.txt=$root/tests/fixtures/set/sub/y.txt"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"sub/y.txt: entry name is not a file beside the manifest"* ]]
}

@test "an absolute entry name is refused" {
    manifest "a.txt=$root/tests/fixtures/set/a.txt" \
        "x.txt=$root/tests/fixtures/set/x.txt" \
        "$root/tests/fixtures/other/x.txt=$root/tests/fixtures/other/x.txt"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"entry name is not a file beside the manifest"* ]]
}

@test "an entry name with a backslash is refused" {
    cp "$root/tests/fixtures/other/x.txt" "$root/tests/fixtures/set/b\\c.txt"
    manifest "a.txt=$root/tests/fixtures/set/a.txt" \
        "x.txt=$root/tests/fixtures/set/x.txt" \
        "b\\\\c.txt=$root/tests/fixtures/set/b\\c.txt"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"entry name is not a file beside the manifest"* ]]
}
