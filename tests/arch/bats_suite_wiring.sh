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
WORKFLOW="${3:-$REPO_ROOT/.github/workflows/bash.yml}"

if [ ! -f "$RUNNER" ]; then
    printf 'arch/bats_suite_wiring: %s is missing\n' "$RUNNER" >&2
    exit 1
fi

# The runner reaching every suite is worth nothing if CI never reaches
# the runner. This is a syntactic constraint — the workflow either
# contains a step invoking `run-suite.sh` or it does not — per the
# AGENTS.md rule on how these checks may read source.
#
# Deliberately not covered, and said so: a step that is present but
# gated behind a condition that never holds, and a line inside a
# block scalar or heredoc that textually mimics the step while being
# payload. Telling either apart from the real thing means parsing
# YAML and then shell — heredocs were the third rung of the ten-round
# ladder the AGENTS.md rule records — so this green means "a line of
# the invocation's exact shape exists", no more. Both evasions
# require a reviewed workflow edit that spells out the mimicry, which
# is what review is for; the relevance-filter cases in the bats suite
# cover the pattern the filter actually uses.
if [ ! -f "$WORKFLOW" ]; then
    printf 'arch/bats_suite_wiring: %s is missing\n' "$WORKFLOW" >&2
    exit 1
fi
# Invocation-shaped, and the whole step: the runner is the entire
# `run:` value aside from whitespace. `run: echo <runner>` names it
# and executes nothing; `run: <runner> || true` executes everything
# and discards the outcome. A match that stopped at the command
# accepted the second.
if ! grep -qE '^[[:space:]]*run:[[:space:]]*(\./)?tests/bash/helpers/run-suite\.sh[[:space:]]*$' \
    "$WORKFLOW"; then
    printf 'arch/bats_suite_wiring FAIL: %s never invokes run-suite.sh — every suite the runner would reach stays unrun in CI\n' \
        "$WORKFLOW" >&2
    exit 1
fi

# Named before it is created, and registered for cleanup before that,
# so an interrupt at any point during setup finds the location already
# known. A name built by a command substitution cannot offer that: the
# directory exists the moment `mktemp` returns and the variable holds
# it only once the substitution completes, and a signal in between
# leaves it behind.
#
# `$$` keeps concurrent runs apart. `ARCH_WIRING_SCRATCH` exists so the
# cases below can hand this a location they control.
scratch="${ARCH_WIRING_SCRATCH:-${TMPDIR:-/tmp}/ani-gui-bats-wiring.$$}"
record="$scratch/invoked"

# Removal is gated on this run having created the directory, not on
# the traps having been installed. The two are not the same instant,
# and between them sits a window in which the handlers are armed and
# nothing here owns anything — long enough for another process to take
# the path and have it deleted on behalf of a run that never made it.
#
# The residue is the reverse: killed between the `mkdir` and the line
# that sets this — or between the flag and the nonce below —
# the run
# leaves a directory behind. That is the trade, and it is the right
# way round. Leaking a directory costs a stale path; removing one
# costs somebody else's data.
owned=""

claim_held=""
cleanup() {
    [ -n "$owned" ] || return 0
    owned=""
    # Deciding and removing have to be one claim. A check on the path
    # followed by an rm removes whatever occupies the path by then —
    # the two can be different directories, and a pid-derived marker
    # can be recreated. The rename takes the occupant out of the
    # shared namespace atomically; the claim file opened at creation
    # and held open on a descriptor says whether it is this run's
    # directory; a stranger's is put back untouched. An occupied
    # reclaim path means leaking ours rather than guessing.
    reclaimed="$scratch.reclaimed.$$"
    if [ -e "$reclaimed" ] || [ -L "$reclaimed" ]; then
        return 0
    fi
    mv "$scratch" "$reclaimed" 2>/dev/null || return 0
    # The window between claiming and removing, held open on request
    # so a case can step into it; the beacon tells the case the claim
    # has been made. Nothing else sets either.
    if [ -n "${ARCH_WIRING_PAUSE_IN_CLEANUP:-}" ]; then
        : >"${ARCH_WIRING_CLEANUP_BEACON:-/dev/null}"
        sleep "$ARCH_WIRING_PAUSE_IN_CLEANUP"
    fi
    # Identity is the open descriptor on the directory itself, not
    # anything stored on disk and not a file. A token written into
    # the directory is readable and therefore copyable; a claim FILE
    # is hard-linkable, and the link is the held inode — either way a
    # replacement can carry the credential. A directory cannot be
    # hard-linked, so `-ef` against the descriptor asks the one
    # question nothing else can answer for: is the reclaimed path the
    # directory this run created. The rename preserves its inode; the
    # descriptor pins it against reuse for as long as the process
    # lives.
    # shellcheck disable=SC3013 # -ef: dash, bash and busybox all provide it, and content comparison is the defect this replaces
    if [ -n "$claim_held" ] && [ -e /dev/fd/9 ] &&
        [ "$reclaimed" -ef /dev/fd/9 ]; then
        rm -rf "$reclaimed"
    elif [ ! -e "$scratch" ] && [ ! -L "$scratch" ]; then
        # Restore only into an absent destination: mv onto an existing
        # directory nests the source inside it, relocating data this
        # run never owned. A path taken again while the stranger's
        # directory was held aside means leaving it at the reclaim
        # path — parked, named, and findable — rather than moving it
        # into somebody else's directory.
        mv "$reclaimed" "$scratch" 2>/dev/null || true
    fi
}

# Registering before creating buys the guarantee above and costs the
# opposite one: a `mkdir` that fails because the path is already taken
# would hand a directory this run never made to `rm -rf`. A pid comes
# round again after a kill that skipped cleanup, so that is reachable
# rather than theoretical.
#
# Both are kept by refusing an occupied path before anything is armed,
# and by disarming on the narrow case where the path was claimed
# between the two. Whatever is there belongs to somebody, and this
# check is not the thing to decide it does not.
if [ -e "$scratch" ]; then
    printf 'arch/bats_suite_wiring: %s already exists — refusing to reuse or remove a location this run did not create\n' \
        "$scratch" >&2
    exit 1
fi

# EXIT owns cleanup; the signal handlers only end the run. `exit`
# inside a trap fires the EXIT trap, so a handler that also called
# cleanup ran it twice — and between the two calls the predictable
# path could be recreated by someone else, handing the second removal
# a directory this run never made.
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

# Between the check above and the `mkdir` below is a window in which
# the traps are armed and this run owns nothing. It is too short to
# step into, so the case that measures it holds it open. Nothing else
# sets this.
if [ -n "${ARCH_WIRING_PAUSE_BEFORE_MKDIR:-}" ]; then
    sleep "$ARCH_WIRING_PAUSE_BEFORE_MKDIR"
fi

if ! mkdir "$scratch" 2>/dev/null; then
    trap - EXIT HUP INT TERM
    printf 'arch/bats_suite_wiring: %s was claimed while this run was starting — refusing to remove a location it did not create\n' \
        "$scratch" >&2
    exit 1
fi
owned=1
# The directory's identity: the directory itself, opened on a
# descriptor this process keeps for its whole life. The open pins the
# inode — a freed inode can be reused by tmpfs, but not while a
# descriptor holds it — and a directory, unlike a file, cannot be
# hard-linked into a replacement, so nothing a same-user process can
# read, copy or link reproduces the identity. The `.claim` file
# remains only as the visible marker the cases synchronise on; it
# carries no authority.
: >"$scratch/.claim"
exec 9<"$scratch"
claim_held=1

# The window after the claim: the directory exists and is owned, and
# whether cleanup acts on the directory this run made or on whatever
# sits at the path by then is the difference the swap case measures.
# Nothing else sets this.
if [ -n "${ARCH_WIRING_PAUSE_AFTER_MKDIR:-}" ]; then
    sleep "$ARCH_WIRING_PAUSE_AFTER_MKDIR"
fi

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
# The record path arrives as data. Spelled into the program text it
# becomes syntax, and a `TMPDIR` holding a double quote then closes a
# string the stub opened — every suite invocation dies of that, and the
# check reports suites that never reached the binary. True, and about a
# cause with nothing to do with the runner.
#
# A quoted heredoc so nothing here expands, and the name is
# `ARCH_`-prefixed because the suite audits these scripts for reads
# from the environment and a generic name would be a real finding.
stub="$scratch/.bats-vendor/bats-core/bin/bats"
cat >"$stub" <<'EOF'
#!/bin/sh
# Every argument has to be a test file. An option changes what runs —
# `--filter` can select zero tests while every file still arrives —
# so the stub refuses anything option-shaped rather than certifying a
# run whose shape it did not understand.
for f in "$@"; do
    case "$f" in
        -*)
            echo "stub: refusing option $f" >&2
            exit 1
            ;;
        *) ;;
    esac
done
for f in "$@"; do
    printf '%s/%s\n' "$(cd "$(dirname "$f")" && pwd)" "$(basename "$f")"
done >>"$ARCH_WIRING_RECORD"
EOF
chmod +x "$stub"
export ARCH_WIRING_RECORD="$record"

runner_name=$(basename "$RUNNER")
cp "$RUNNER" "$scratch/helpers/$runner_name"

# Every `.bats` file that really exists, at the path it really sits at.
# One probe per directory would only establish that each directory was
# reached: a runner keeping every directory and dropping all but the
# first file in each satisfies that exactly, while most of a suite
# stops running.
#
# The stub never opens them, so the contents are immaterial. What
# matters is the shape of the tree the runner walks.
expected="$scratch/expected"
: >"$expected"
suites=0
files=0
for dir in "$SUITES_DIR"/*/; do
    [ -d "$dir" ] || continue
    name=$(basename "$dir")
    case "$name" in
        .bats-vendor | helpers) continue ;;
        *) ;;
    esac
    mirrored=0
    find "$dir" -type f -name '*.bats' 2>/dev/null | while IFS= read -r real; do
        printf '%s\n' "${real#"$dir"}"
    done >"$scratch/.relative"
    while IFS= read -r rel; do
        [ -n "$rel" ] || continue
        if [ "$mirrored" -eq 0 ]; then
            mkdir "$scratch/$name"
            mirrored=1
            suites=$((suites + 1))
        fi
        case "$rel" in
            */*) mkdir -p "$scratch/$name/${rel%/*}" ;;
            *) ;;
        esac
        : >"$scratch/$name/$rel"
        printf '%s/%s\n' \
            "$(cd "$scratch/$name/$(dirname "$rel")" && pwd)" \
            "$(basename "$rel")" >>"$expected"
        files=$((files + 1))
    done <"$scratch/.relative"
done
rm -f "$scratch/.relative"

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

# Every mirrored file, not every mirrored directory. The expected list
# was written as the tree was built, so it needs no second walk and no
# second copy of the exclusion rules.
failed=0
while IFS= read -r want <&3; do
    [ -n "$want" ] || continue
    found=0
    while IFS= read -r invoked; do
        if [ "$invoked" = "$want" ]; then
            found=1
            break
        fi
    done <"$record"
    if [ "$found" -eq 0 ]; then
        printf 'arch/bats_suite_wiring FAIL: %s never reached the bats binary — the runner does not name its suite, skips it in the loop body, or drops the file from the list it passes on, and in every case those cases do not run\n' \
            "${want#"$scratch"/}" >&2
        failed=1
    fi
done 3<"$expected"

[ "$failed" -eq 0 ] || exit 1
printf 'arch/bats_suite_wiring: ok (%d bats files across %d suites, every one reaching the bats binary)\n' \
    "$files" "$suites"
