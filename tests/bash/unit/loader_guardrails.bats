#!/usr/bin/env bats
# source_ani_cli_lib must not disarm bats' failure detection for the
# rest of the test: historically it stripped errexit/errtrace and the
# ERR trap for the whole test, so a failing command or assertion after
# the source was ignored and the test's outcome was whatever the LAST
# command returned — a silent-pass gap that let broken assertions ride
# green suites.
#
# Pin the contract black-box: run a nested bats on a fixture whose
# test fails right after source_ani_cli_lib and then ends with a
# passing command. The inner run must report the failure. (White-box
# probes of $- and `trap -p` are misleading here — bash suppresses
# errexit in tested contexts and reports parent traps from command
# substitutions — so only the observable outcome is trustworthy.)

load '../helpers/loader'

write_gap_fixture() {
    # $1: the failing line to embed after source_ani_cli_lib.
    cat > "$BATS_TEST_TMPDIR/gap.bats" <<EOF
load '$REPO_ROOT/tests/bash/helpers/loader'

@test "must not be swallowed" {
    source_ani_cli_lib
    $1
    true
}
EOF
}

@test "a failing bare command after source_ani_cli_lib fails the test" {
    write_gap_fixture "false"
    run -1 "$BATS_VENDOR/bats-core/bin/bats" "$BATS_TEST_TMPDIR/gap.bats"
    assert_output --partial "not ok 1"
}

@test "a failing assertion after source_ani_cli_lib fails the test" {
    write_gap_fixture 'assert_equal "a" "b"'
    run -1 "$BATS_VENDOR/bats-core/bin/bats" "$BATS_TEST_TMPDIR/gap.bats"
    assert_output --partial "not ok 1"
}

@test "a passing test that sources the lib still passes" {
    # Guard the other direction: restoring failure detection must not
    # start failing tests on the innocuous non-zero lines inside
    # ani-cli's setup block or on lib functions used normally.
    cat > "$BATS_TEST_TMPDIR/ok.bats" <<EOF
load '$REPO_ROOT/tests/bash/helpers/loader'

@test "sources and calls a lib function" {
    source_ani_cli_lib
    run nth "1" <<<"1	one"
    assert_success
}
EOF
    run -0 "$BATS_VENDOR/bats-core/bin/bats" "$BATS_TEST_TMPDIR/ok.bats"
    assert_output --partial "ok 1"
}
