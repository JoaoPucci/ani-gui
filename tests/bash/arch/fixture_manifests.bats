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

@test "a Windows drive-relative entry name is refused" {
    # On Windows `C:x.txt` names x.txt in the current directory of
    # drive C, wherever that is, not a file beside the manifest.
    rm "$root/tests/fixtures/set/x.txt"
    cp "$root/tests/fixtures/set/a.txt" "$root/tests/fixtures/set/C:x.txt"
    manifest "a.txt=$root/tests/fixtures/set/a.txt" \
        "C:x.txt=$root/tests/fixtures/set/C:x.txt"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"C:x.txt: entry name is not a file beside the manifest"* ]]
}

@test "an entry that is a symbolic link is refused" {
    # The name is a plain file name, but the link reaches outside the
    # manifest's directory, so its digest would vouch for another file.
    rm "$root/tests/fixtures/set/x.txt"
    ln -s ../other/x.txt "$root/tests/fixtures/set/link.txt"
    manifest "a.txt=$root/tests/fixtures/set/a.txt" \
        "link.txt=$root/tests/fixtures/other/x.txt"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"link.txt: is a symbolic link"* ]]
}

@test "an unlisted symbolic link beside a manifest is refused" {
    rm "$root/tests/fixtures/set/x.txt"
    ln -s ../other/x.txt "$root/tests/fixtures/set/link.txt"
    manifest "a.txt=$root/tests/fixtures/set/a.txt"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"link.txt: is a symbolic link"* ]]
}

@test "a subdirectory beside a manifest with no manifest of its own is refused" {
    rm "$root/tests/fixtures/set/x.txt"
    mkdir "$root/tests/fixtures/set/sub"
    printf 'deeper\n' >"$root/tests/fixtures/set/sub/y.txt"
    manifest "a.txt=$root/tests/fixtures/set/a.txt"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"sub: is a directory with no MANIFEST.json"* ]]
}

@test "a subdirectory beside a manifest with a manifest of its own passes" {
    rm "$root/tests/fixtures/set/x.txt"
    mkdir "$root/tests/fixtures/set/sub"
    printf 'deeper\n' >"$root/tests/fixtures/set/sub/y.txt"
    printf '{"y.txt": {"sha256": "%s"}}\n' "$(sha "$root/tests/fixtures/set/sub/y.txt")" \
        >"$root/tests/fixtures/set/sub/MANIFEST.json"
    manifest "a.txt=$root/tests/fixtures/set/a.txt"
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
    [[ "$output" == *"OK (2 manifests)"* ]]
}

@test "a symbolic link to a directory is not followed into a manifest elsewhere" {
    # The linked directory's manifest is wrong; following the link would
    # report it as though it were part of tests/fixtures/.
    mkdir -p "$root/outside"
    printf 'out\n' >"$root/outside/z.txt"
    printf '{"z.txt": {"sha256": "%s"}}\n' "$(sha "$root/tests/fixtures/set/a.txt")" \
        >"$root/outside/MANIFEST.json"
    manifest "a.txt=$root/tests/fixtures/set/a.txt" "x.txt=$root/tests/fixtures/set/x.txt"
    ln -s ../../outside "$root/tests/fixtures/linked"
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
    [ "$status" -eq 0 ]
    [[ "$output" != *"outside"* && "$output" != *"linked/"* ]]
}

@test "a MANIFEST.json that is a symbolic link is refused" {
    manifest "a.txt=$root/tests/fixtures/set/a.txt" "x.txt=$root/tests/fixtures/set/x.txt"
    mv "$root/tests/fixtures/set/MANIFEST.json" "$root/tests/fixtures/other/real.json"
    ln -s ../other/real.json "$root/tests/fixtures/set/MANIFEST.json"
    ARCH_REPO_ROOT="$root" run ! sh "$CHECK"
    [[ "$output" == *"set/MANIFEST.json: is a symbolic link"* ]]
}
