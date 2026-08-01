#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC2312 — command substitutions are read for their text, and a
#       failure arrives as an empty result that the assertion catches
#
# shellcheck disable=SC2312
# Architectural invariant: every bats suite is actually invoked.
#
# `tests/bash/helpers/run-suite.sh` decides which suites run. A
# directory full of `.bats` files it never hands to the bats binary is
# never run, and nothing says so — the job passes having executed the
# suites it does know about.
#
# Reading the runner's text cannot establish that, and two readings
# have already been tried and found weaker than the property. Being
# named in the loop's list is not being executed. A loop body that
# mentions the bats binary somewhere is not every suite reaching it —
# one `[ "$suite" = arch ] && continue` before the invocation leaves
# both readings intact while the arch cases stop running.
#
# So this runs the runner. A sandbox mimics `tests/bash/` with one
# probe file per real suite and a recording stub where the bats binary
# would be, and every suite has to arrive at that stub. What the check
# asserts is then the thing it is named for, and it holds against a
# runner rewritten into a shape no parser here anticipated.
#
# This cannot be checked from inside the bats suites themselves. A test
# that has not run cannot report its own absence, so a case under
# `tests/bash/arch/` asserting the arch suite is wired is satisfied
# only when the thing it doubts is already true. The check has to live
# where it runs unconditionally, which is here: `run-all.sh` executes
# every `tests/arch/*.sh` on every pull request.

set -eu

REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$REPO_ROOT"

RUNNER="${1:-$REPO_ROOT/tests/bash/helpers/run-suite.sh}"
SUITES_DIR="${2:-$REPO_ROOT/tests/bash}"

if [ ! -f "$RUNNER" ]; then
    printf 'arch/bats_suite_wiring: %s is missing\n' "$RUNNER" >&2
    exit 1
fi

# Named before it is created, and registered for cleanup before that,
# so an interrupt at any point during setup finds the location already
# known. A name built by a command substitution cannot offer that: the
# directory exists the moment `mktemp` returns and the variable holds
# it only once the substitution completes, and a signal in between
# leaves it behind.
#
# `$$` keeps concurrent runs apart, and plain `mkdir` refuses a path
# that already exists — including a symlink planted at it.
scratch="${TMPDIR:-/tmp}/ani-gui-bats-wiring.$$"
record="$scratch/invoked"

cleanup() {
    rm -rf "$scratch"
}
trap cleanup EXIT
trap 'cleanup; exit 130' INT
trap 'cleanup; exit 143' TERM

mkdir "$scratch"
mkdir "$scratch/helpers"
mkdir -p "$scratch/.bats-vendor/bats-core/bin"
: >"$record"

# The runner resolves everything from its own location, so a copy
# placed in the sandbox's `helpers/` finds the sandbox's suites and the
# sandbox's bats. Nothing here can reach the real vendored binary or
# run a real case.
#
# What gets recorded is the directory each file resolves to, not the
# path as written. The runner reaches its suites through its own
# location — `helpers/../unit` — so comparing the strings answers a
# question about spelling rather than about which suite ran.
stub="$scratch/.bats-vendor/bats-core/bin/bats"
cat >"$stub" <<EOF
#!/bin/sh
for f in "\$@"; do
    (cd "\$(dirname "\$f")" && pwd)
done >>"$record"
EOF
chmod +x "$stub"

runner_name=$(basename "$RUNNER")
cp "$RUNNER" "$scratch/helpers/$runner_name"

# One probe per suite that really exists. The stub never opens them, so
# the contents are immaterial — what matters is that the runner's own
# "any bats files here?" test says yes for exactly the directories the
# repository expects to run.
suites=0
for dir in "$SUITES_DIR"/*/; do
    [ -d "$dir" ] || continue
    name=$(basename "$dir")
    case "$name" in
        .bats-vendor | helpers) continue ;;
        *) ;;
    esac
    if [ -z "$(find "$dir" -type f -name '*.bats' 2>/dev/null)" ]; then
        continue
    fi
    mkdir "$scratch/$name"
    : >"$scratch/$name/probe.bats"
    suites=$((suites + 1))
done

# With nothing to require, every runner satisfies this check. That is
# an absence of evidence rather than a pass, and it has to read as one.
if [ "$suites" -eq 0 ]; then
    printf 'arch/bats_suite_wiring FAIL: no bats suites found under %s — this check has lost its subject and would report ok against any runner\n' \
        "$SUITES_DIR" >&2
    exit 1
fi

runner_out="$scratch/runner.out"
if ! sh "$scratch/helpers/$runner_name" >"$runner_out" 2>&1; then
    printf 'arch/bats_suite_wiring FAIL: %s exited non-zero against a stub that passes everything — it should have walked every suite and reported success\n' \
        "$RUNNER" >&2
    sed 's/^/    /' "$runner_out" >&2
    exit 1
fi

failed=0
for dir in "$SUITES_DIR"/*/; do
    [ -d "$dir" ] || continue
    name=$(basename "$dir")
    # The probe marks the directories the first pass mirrored, which is
    # the exclusion list already applied rather than a second copy of
    # it. `helpers/` exists in the sandbox too — it holds the runner —
    # and carries no probe.
    [ -f "$scratch/$name/probe.bats" ] || continue
    want=$(cd "$scratch/$name" && pwd)
    found=0
    while IFS= read -r invoked; do
        if [ "$invoked" = "$want" ]; then
            found=1
            break
        fi
    done <"$record"
    if [ "$found" -eq 0 ]; then
        printf 'arch/bats_suite_wiring FAIL: %s holds bats files and never reached the bats binary — the runner either does not name it or skips it in the loop body, and either way those cases do not run\n' \
            "$dir" >&2
        failed=1
    fi
done

[ "$failed" -eq 0 ] || exit 1
printf 'arch/bats_suite_wiring: ok (%d bats suites, every one reaching the bats binary)\n' "$suites"
