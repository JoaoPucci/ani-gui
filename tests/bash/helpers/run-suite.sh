#!/bin/sh
# Run the bash test suite — today the arch harness under
# tests/bash/arch/ — using the vendored bats. CI invokes this.
# Locally, run it after install-bats.sh.

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENDOR_DIR="$SCRIPT_DIR/../.bats-vendor"
BATS_BIN="$VENDOR_DIR/bats-core/bin/bats"
TESTS_BASH_DIR="$SCRIPT_DIR/.."

if [ ! -x "$BATS_BIN" ]; then
    printf 'bats not installed; run %s first\n' "$SCRIPT_DIR/install-bats.sh" >&2
    exit 1
fi

# Run each suite separately so failures are easier to locate. `bats` exits
# non-zero on the first failing suite; we keep going so the developer sees
# every failure at once, then exit non-zero at the end if any suite failed.
overall=0
# The list holds one entry since the vendored script's suites left
# with their subject. It stays a loop all the same: the wiring check's
# sabotage cases operate on this line and the skip inside the body,
# and adding a suite back is a one-word change.
# shellcheck disable=SC2043
for suite in arch; do
    dir="$TESTS_BASH_DIR/$suite"
    if [ ! -d "$dir" ]; then
        continue
    fi
    files=$(find "$dir" -type f -name '*.bats' | sort)
    if [ -z "$files" ]; then
        printf '  (no bats files in %s yet, skipping)\n' "$suite"
        continue
    fi
    # Filenames reach bats as separate arguments rather than as one
    # word-split string. Split, a checkout under `~/My Repos/` hands
    # bats the pieces of every path and runs nothing.
    set --
    while IFS= read -r bats_file; do
        [ -n "$bats_file" ] || continue
        set -- "$@" "$bats_file"
    done <<EOF
$files
EOF

    printf '\n=== suite: %s ===\n' "$suite"
    if ! "$BATS_BIN" "$@"; then
        overall=1
    fi
done

exit "$overall"
