#!/usr/bin/env bats
#
# Tests for ani-cli's `backup_old_hist` (new in 5.0).
#
# 5.0's history rows are keyed on anidb slugs ("one-piece-69"); every
# earlier version wrote provider-native ids with no hyphen ("ReooPAxP…").
# The -c path migrates once on entry: rows whose id contains a hyphen
# stay, every other row moves to ${histfile}.v4, and the rewrite is
# atomic through ${histfile}.new.
#
# This is the script's own history and nobody else's. The GUI kept its
# watch history in this file in the allanime era and now keeps its own,
# so the split shape here is what a terminal user sees and not what the
# backend reads.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    histfile="$BATS_TEST_TMPDIR/hist/ani-hsts"
}

@test "backup_old_hist: copies an all-pre-5.0 file to the .v4 backup" {
    # Upstream wart, pinned as observed: when NO row survives, the
    # .new rewrite is never created, the mv fails, and the histfile
    # keeps its old rows alongside the backup. The -c flow shrugs this
    # off (the unmigrated ids then just resolve no episodes), so the
    # pin documents what 5.0 does, not what it should do.
    printf '3\tReooPAxPMsHM4KPMY\tOne Piece (1122 episodes)\n' >"$histfile"
    backup_old_hist 2>/dev/null || true
    [ -f "${histfile}.v4" ]
    grep -F "ReooPAxPMsHM4KPMY" "${histfile}.v4" >/dev/null
}

@test "backup_old_hist: keeps slug rows in place" {
    printf '2\tone-piece-69\tOne Piece\n' >"$histfile"
    backup_old_hist
    grep -F "one-piece-69" "$histfile" >/dev/null
    [ ! -f "${histfile}.v4" ]
}

@test "backup_old_hist: splits a mixed file by id shape" {
    {
        printf '3\tReooPAxPMsHM4KPMY\tOne Piece (1122 episodes)\n'
        printf '2\tone-piece-69\tOne Piece\n'
        printf '12\tabc123XYZ\tAttack on Titan (25 episodes)\n'
    } >"$histfile"
    backup_old_hist
    line_count=$(wc -l <"$histfile" | tr -d ' ')
    [ "$line_count" -eq 1 ]
    grep -F "one-piece-69" "$histfile" >/dev/null
    backup_count=$(wc -l <"${histfile}.v4" | tr -d ' ')
    [ "$backup_count" -eq 2 ]
}

@test "backup_old_hist: leaves no .new sidecar behind" {
    printf '2\tone-piece-69\tOne Piece\n' >"$histfile"
    backup_old_hist
    [ ! -f "${histfile}.new" ]
}
