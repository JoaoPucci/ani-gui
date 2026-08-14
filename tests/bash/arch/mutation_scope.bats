#!/usr/bin/env bats
#
# Whether the mutation-scope check can tell a glob that selects a
# module from one that selects nothing.
#
# The property is asymmetric, and only one side of it is visible in a
# passing run. A glob naming a live directory and a glob naming a
# deleted one produce the same green from `cargo mutants`; the second
# just contributes no mutants. So the case that matters is the
# negative one — the check has to go red on a scope that has quietly
# emptied — and a check asserted only against the repository as it
# stands is satisfied by returning zero unconditionally.
#
# Every case below therefore hands the real check a workflow and a
# crate it constructs, and reads the exit status. The subject is run;
# nothing here re-implements what it does.

load '../helpers/loader'

setup() {
    CHECK="$REPO_ROOT/tests/arch/mutation_scope.sh"
    CRATE="$BATS_TEST_TMPDIR/crate"
    mkdir -p "$CRATE/src/proxy" "$CRATE/src/history"
}

# A workflow whose mutation step globs exactly the given paths, laid
# out the way the real one is: an indented `-f 'glob' \` per line
# inside a block scalar.
workflow_globbing() {
    path=$(mktemp "$BATS_TEST_TMPDIR/workflow-XXXXXX.yml")
    {
        printf 'jobs:\n  cargo-mutants:\n    steps:\n'
        printf '      - name: Run cargo-mutants on scoped modules\n'
        printf '        working-directory: gui/backend\n'
        printf '        run: |\n          cargo mutants \\\n'
        for glob in "$@"; do
            printf "            -f '%s' \\\\\n" "$glob"
        done
        printf '            --json > mutants.json\n'
    } >"$path"
    printf '%s\n' "$path"
}

@test "the repository's own mutation scope resolves" {
    # The real workflow against the real crate. This is the case that
    # goes red when someone moves a module and leaves the glob behind.
    run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "a glob naming a live directory passes" {
    wf=$(workflow_globbing 'src/proxy/**/*.rs' 'src/history/**/*.rs')
    run sh "$CHECK" "$wf" "$CRATE"
    [ "$status" -eq 0 ]
}

@test "a glob naming a directory that no longer exists is caught" {
    # The regression this check exists for: a module is renamed, the
    # workflow keeps globbing the old path, and the nightly run goes on
    # reporting success while mutating none of it.
    wf=$(workflow_globbing 'src/proxy/**/*.rs' 'src/anicli/**/*.rs')
    run sh "$CHECK" "$wf" "$CRATE"
    [ "$status" -ne 0 ]
    [[ "$output" == *'src/anicli'* ]]
}

@test "the failure names the glob and the path it resolved to" {
    # A check that fails without saying which of four globs went stale
    # sends the reader back to diff the workflow by hand.
    wf=$(workflow_globbing 'src/gone/**/*.rs')
    run sh "$CHECK" "$wf" "$CRATE"
    [ "$status" -ne 0 ]
    [[ "$output" == *'src/gone/**/*.rs'* ]]
    [[ "$output" == *"$CRATE/src/gone"* ]]
}

@test "every stale glob is reported, not just the first" {
    # Stopping at the first would hide the rest behind a fix-and-rerun
    # cycle, one round per glob.
    wf=$(workflow_globbing 'src/gone/**/*.rs' 'src/also-gone/**/*.rs')
    run sh "$CHECK" "$wf" "$CRATE"
    [ "$status" -ne 0 ]
    [[ "$output" == *'src/gone'* ]]
    [[ "$output" == *'src/also-gone'* ]]
}

@test "a bare file path is resolved as a file" {
    # `-f` takes a glob, but a glob with no wildcard in it is just a
    # path, and it goes stale the same way.
    : >"$CRATE/src/proxy/mod.rs"
    wf=$(workflow_globbing 'src/proxy/mod.rs')
    run sh "$CHECK" "$wf" "$CRATE"
    [ "$status" -eq 0 ]

    wf=$(workflow_globbing 'src/proxy/absent.rs')
    run sh "$CHECK" "$wf" "$CRATE"
    [ "$status" -ne 0 ]
}

@test "a workflow that declares no globs is refused, not certified" {
    # With nothing to resolve there is nothing to be right about, and a
    # zero exit here would read as a scope that checks out. A `-f` list
    # deleted wholesale is exactly how the scope reaches empty.
    empty="$BATS_TEST_TMPDIR/no-globs.yml"
    printf 'jobs:\n  cargo-mutants:\n    steps: []\n' >"$empty"
    run sh "$CHECK" "$empty" "$CRATE"
    [ "$status" -ne 0 ]
    [[ "$output" == *'no -f globs'* ]]
}

@test "a missing workflow is refused, not skipped" {
    run sh "$CHECK" "$BATS_TEST_TMPDIR/absent.yml" "$CRATE"
    [ "$status" -ne 0 ]
}

@test "a missing crate root is refused, not skipped" {
    wf=$(workflow_globbing 'src/proxy/**/*.rs')
    run sh "$CHECK" "$wf" "$BATS_TEST_TMPDIR/no-such-crate"
    [ "$status" -ne 0 ]
}
