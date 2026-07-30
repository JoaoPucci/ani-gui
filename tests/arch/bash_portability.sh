#!/bin/sh
# Architectural invariant: the upstream `ani-cli` script is POSIX sh and
# must never use awk (per upstream's own CI gate `check-no-awk`). Our
# carried changes (just the __ANI_CLI_LIB__ source-guard) must respect
# that constraint.
#
# This script is a thin mirror of upstream's `! grep awk "./ani-cli"`
# check in .github/workflows/ani-cli.yml — duplicated locally so
# `bash tests/arch/run-all.sh` catches a regression before CI does.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -f ani-cli ]; then
    printf 'arch/bash_portability: ani-cli script not present — skipping\n'
    exit 0
fi

failed=0

# 1. No awk in the script.
if grep -q '\bawk\b' ani-cli; then
    matches=$(grep -n '\bawk\b' ani-cli)
    printf 'arch/bash_portability FAIL: ani-cli contains awk:\n%s\n' "$matches" >&2
    failed=1
fi

# 2. Shebang must be /bin/sh.
first_line=$(head -n1 ani-cli)
if [ "$first_line" != "#!/bin/sh" ]; then
    printf 'arch/bash_portability FAIL: ani-cli shebang is %s (expected #!/bin/sh)\n' "$first_line" >&2
    failed=1
fi

# 3. Diff against upstream master should differ by no more than one line
# (the carried __ANI_CLI_LIB__ guard). Skip if no upstream remote is
# configured.
if git remote get-url upstream >/dev/null 2>&1; then
    # An immutable tag, not a branch. Fetching `master` makes the
    # baseline move under us: the count changes when upstream commits,
    # so a branch that touched nothing goes red, and the first reaction
    # to a red with no local cause is to raise the ceiling. That is
    # how it reached 4 against a real 41.
    #
    # The tag is the release the vendored script came from. Bumping it
    # is part of syncing the script, which is where that decision
    # belongs.
    UPSTREAM_BASELINE=v4.15
    if git fetch upstream "tag $UPSTREAM_BASELINE" --no-tags --quiet 2>/dev/null \
        || git rev-parse -q --verify "$UPSTREAM_BASELINE" >/dev/null; then
        diff_lines=$(git diff "$UPSTREAM_BASELINE" -- ani-cli | grep -cE '^[+-][^+-]' || true)
        # Each carried line shows as one + or - in the diff. The
        # ceiling is 50, which is the carried patch set AGENTS.md §3
        # lists — the source guard, the greedy name capture, the
        # watched-episode fallback, the portable base64, and the
        # flatpak player acceptance — with room for one more small
        # one before this has to be looked at again.
        #
        # It was 4, describing only the guard, and the real diff has
        # been 41 for some time. A ceiling that everything fails
        # against stops being a signal, so raising it to match the
        # patches we mean to carry is the repair; lowering it back is
        # what the native resolver eventually does by deleting the
        # script.
        if [ "$diff_lines" -gt 50 ]; then
            printf 'arch/bash_portability FAIL: ani-cli diverges from upstream by %d lines (max 50) — a patch beyond the set in AGENTS.md §3 has landed, or one grew\n' "$diff_lines" >&2
            failed=1
        fi
    fi
fi

if [ "$failed" -eq 0 ]; then
    printf 'arch/bash_portability PASS\n'
fi
exit "$failed"
