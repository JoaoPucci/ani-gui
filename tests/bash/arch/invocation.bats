#!/usr/bin/env bats
#
# How the arch checks resolve the repository, and whether the linter
# actually looks at them.
#
# Two properties, both broken in turn by the same seam. A check that
# re-derives its own root from `$0` walks above the repository once a
# caller has changed directory; honouring an inherited `REPO_ROOT`
# fixed that and introduced the other half, since `REPO_ROOT` is a
# name any shell may already have set — and a redirected check exits 0
# against the wrong tree, reporting success about a repository nobody
# asked it to inspect.

load '../helpers/loader'

setup() {
    ARCH_DIR="$REPO_ROOT/tests/arch"
    WORKFLOW="$REPO_ROOT/.github/workflows/arch-lint.yml"
    BASH_WORKFLOW="$REPO_ROOT/.github/workflows/bash.yml"
}

# Run a script by relative path from a directory, passing both as
# arguments rather than pasting them into a shell program. A path is
# data; the moment it is interpolated into a command string it becomes
# syntax, and a checkout under `o'brien/` then closes a quote the
# string opened. `run` already forks, so the `cd` cannot leak into the
# rest of the test.
run_from() {
    cd "$1" || return 1
    shift
    sh "$@"
}

@test "each check resolves the repository when invoked by relative path" {
    for script in deferral_record agents_contract; do
        run run_from "$ARCH_DIR" "./$script.sh"
        [ "$status" -eq 0 ]
    done
}

@test "a check runs from a directory whose name contains an apostrophe" {
    # Nothing stops anyone cloning into `~/o'brien/`. When the path is
    # built into a shell program instead of passed to one, the
    # apostrophe ends the quoting and every case in this file dies of a
    # syntax error before a check is reached — reporting a failure that
    # names the shell rather than the path. The self-test this file
    # replaced carried a fixture for exactly that.
    quoted="$BATS_TEST_TMPDIR/o'brien"
    mkdir -p "$quoted"
    printf '#!/bin/sh\nexit 0\n' >"$quoted/trivial.sh"
    run run_from "$quoted" ./trivial.sh
    [ "$status" -eq 0 ]
}

@test "a stray REPO_ROOT does not redirect a check" {
    # `REPO_ROOT` is a common name. Only the suite's own
    # `ARCH_REPO_ROOT` may point a check somewhere else.
    for script in deferral_record agents_contract; do
        run env REPO_ROOT=/tmp sh "$ARCH_DIR/$script.sh"
        [ "$status" -eq 0 ]
    done
}

@test "the sourced library keeps the caller's resolved root" {
    # Sourced, `$0` is whatever the sourcing shell was invoked as, so
    # re-deriving from it lands outside the repository altogether.
    # Executed, the same line is correct — which is why running the
    # check by hand never showed this.
    probe="$BATS_TEST_TMPDIR/probe-repo"
    mkdir -p "$probe"
    (cd "$probe" && git init -q .) >/dev/null 2>&1
    run env ARCH_DEFERRAL_RECORD_LIB=1 ARCH_REPO_ROOT="$probe" \
        sh -c '. "$1/tests/arch/deferral_record.sh"; pwd' _ "$REPO_ROOT"
    [[ "$output" == "$probe"* ]]
}

# Whether a workflow fires unconditionally: it exists and its trigger
# block names no paths. A filter would reopen two gaps at once — a
# pull request the filter misses never lints the arch scripts, and the
# stub-mirror pair that papers over the zero-diff case is a second
# producer of the check name, able to answer for a lint that failed.
unconditional() {
    [ -f "$1" ] || return 1
    ! sed -n '/^on:/,/^[a-z]/p' "$1" | grep -q 'paths'
}

# The exclusion extraction and its refusal, as functions so a fixture
# spelling runs through exactly the code the live line runs through.
#
# Quotes are stripped only in pairs: the first two substitutions each
# take a value whose quote closes on the line it opened on, and the
# unquoted plain form falls through to the last. A quote that opens
# and never closes matches neither pair, the third command leaves the
# line untouched, and the surviving key text lands in the refusal —
# a scalar continued on the next physical line must not read as the
# fragment before the break.
parse_exclusions() {
    sed "s/.*sh_checker_exclude:[[:space:]]*\"\([^\"]*\)\".*/\1/; t
s/.*sh_checker_exclude:[[:space:]]*'\([^']*\)'.*/\1/; t
/sh_checker_exclude:[[:space:]]*[\"']/b
s/.*sh_checker_exclude:[[:space:]]*//"
}

# Three ways an extraction fails, all refused: the key text survives
# into the value, the value opens with YAML syntax — block scalar,
# flow collection, anchor or alias — meaning the real list lives
# somewhere this line-oriented read never looked, or the value carries
# an escape the extraction does not resolve, so the tokens as read are
# not the tokens the action receives. An empty value is not refused: a
# key with nothing after it excludes nothing, and that is a correct
# read.
exclusions_unreadable() {
    case "$1" in
        *sh_checker_exclude*) return 0 ;;
        '>'* | '|'* | '['* | '{'* | '&'* | '*'*) return 0 ;;
        *\\*) return 0 ;;
        *) return 1 ;;
    esac
}

# Whether a check can be redirected by the environment it runs in.
#
# Two have been, for real: `REPO_ROOT` and `SKIP_NESTED` each arrived
# from an ordinary shell and silently changed what a check did while
# it still exited 0. The property is held by running each check rather
# than by reading it for suspicious names — deciding from patterns
# which `$NAME` is a read and which assignment owns it needs a shell
# parser, and building one out of regular expressions cost ten review
# rounds before it was replaced by this. See AGENTS.md §2.
hostile_env() {
    env \
        REPO_ROOT=/nonexistent-hostile-root \
        SKIP_NESTED=1 \
        GENERIC_GUARD=1 \
        ROOT=/nonexistent \
        DIR=/nonexistent \
        FILE=/nonexistent \
        DEBUG=1 VERBOSE=1 QUIET=1 FORCE=1 DRY_RUN=1 \
        "$@"
}

# Sensitive when the hostile environment changes what the check prints
# or how it exits. A clean run happens twice first: a check that is not
# reproducible against itself — one that prints durations — differs
# from anything, and for those the exit status is the only stable
# signal.
env_sensitive() {
    _first=$(sh "$1" 2>&1) && _first_status=0 || _first_status=$?
    _again=$(sh "$1" 2>&1) || true
    _dirty=$(hostile_env sh "$1" 2>&1) && _dirty_status=0 || _dirty_status=$?

    [ "$_first_status" = "$_dirty_status" ] || return 0
    if [ "$_first" = "$_again" ] && [ "$_first" != "$_dirty" ]; then
        return 0
    fi
    return 1
}

@test "a check a stray environment redirects is detected" {
    # The hunt has to find something it is known to find, or the day
    # it stops working it reports ok about every check at once.
    env_sensitive "$REPO_ROOT/tests/fixtures/arch/redirectable-check.sh"
}

@test "no check changes what it does under a hostile environment" {
    strays=''
    for check in "$ARCH_DIR"/*.sh; do
        case "$(basename "$check")" in
            run-all.sh) continue ;;
            *) ;;
        esac
        if env_sensitive "$check"; then
            strays="$strays $(basename "$check")"
        fi
    done
    [ -z "$strays" ] || {
        echo "a stray environment redirects:$strays"
        return 1
    }
}

@test "exactly one workflow reports the arch lint check name" {
    # The green has to be attributable. `Shellcheck + Shfmt` is a name
    # three workflows report, two of them succeeding without reading
    # these files, and branch protection accepts the first success
    # under a required name — so the arch lint owns a name nothing
    # else answers for.
    count=$(cat "$REPO_ROOT/.github/workflows/"*.yml |
        grep -c 'name:[[:space:]]*Arch Shellcheck + Shfmt' || true)
    [ "$count" -eq 1 ]
}

@test "the arch lint workflow fires unconditionally" {
    unconditional "$WORKFLOW"
}

@test "a path-gated workflow does not count as unconditional" {
    # The fixture is the workflow with a filter added — what a
    # well-meaning edit narrowing CI would leave behind. The helper
    # has to reject it, or the case above certifies a lint that can
    # be skipped.
    fixture="$BATS_TEST_TMPDIR/gated.yml"
    cat >"$fixture" <<'YAML'
on:
  pull_request:
    paths:
      - "tests/arch/**.sh"
jobs:
  lint:
    runs-on: ubuntu-latest
YAML
    run ! unconditional "$fixture"
}

@test "the linter does not exclude tests/arch from checking" {
    # Starting the job is not the same as inspecting anything. The
    # action takes its own exclude list, and a bare `tests` there skips
    # these files after the workflow has started for them. A value the
    # extraction cannot read fails here too — refused, not scanned.
    line=$(grep 'sh_checker_exclude' "$WORKFLOW" | head -1)
    tokens=$(printf '%s' "$line" | parse_exclusions)
    run ! exclusions_unreadable "$tokens"
    subject=tests/arch
    for token in $tokens; do
        case "$subject" in
            "$token" | "$token"/*) return 1 ;;
            *) ;;
        esac
    done
}

@test "the exclusion extraction survives a single-quoted value" {
    # Stripping only double quotes leaves a single-quoted value
    # carrying its quote characters; the tokens then match nothing and
    # the case above reports the scripts linted while the action still
    # excludes them.
    probe=$(printf '%s\n' "      sh_checker_exclude: 'tests/probe other'" |
        parse_exclusions)
    [ "$probe" = 'tests/probe other' ]
}

@test "a block-scalar exclusion is refused, not scanned" {
    # `sh_checker_exclude: >-` keeps the value on the next line; the
    # extraction returns the fold marker, no key text survives, and an
    # exclude list that does cover tests/arch would scan as not
    # covering it. Past-the-boundary spellings arrive as refusals.
    probe=$(printf '%s\n' '      sh_checker_exclude: >-' | parse_exclusions)
    exclusions_unreadable "$probe"
}

@test "an unterminated quoted exclusion is refused, not scanned" {
    # YAML continues a quoted scalar onto the next physical line:
    # `sh_checker_exclude: "ani-cli` with `tests/arch"` beneath it
    # resolves to one list, while a line-oriented read of the first
    # line extracts a valid-looking fragment — no key text, no YAML
    # syntax, no escape for the refusal arms to catch, and missing
    # exactly the entry that mattered. A quote that opens on the line
    # and never closes means the value is not on the line.
    probe=$(printf '%s\n' '      sh_checker_exclude: "ani-cli' |
        parse_exclusions)
    exclusions_unreadable "$probe"
}

@test "an escaped exclusion is refused, not scanned" {
    # Double-quoted YAML resolves escapes: "tests\x2farch" reaches the
    # action as tests/arch and excludes these scripts. The extraction
    # keeps the backslash, the token then matches nothing, and the
    # scan reports the scripts linted. Escapes are YAML the extraction
    # does not resolve — refused, like the other spellings it cannot
    # read.
    probe=$(printf '%s\n' '      sh_checker_exclude: "ani-cli tests\x2farch"' |
        parse_exclusions)
    exclusions_unreadable "$probe"
}

@test "the bats job runs when a check these tests cover changes" {
    # These tests exercise `tests/arch/*.sh`. The job that runs them
    # decides relevance from the changed paths, so a change to a check
    # and nothing else has to count — otherwise editing the very thing
    # under test skips the suite, and the pull request goes green
    # having run none of it.
    #
    # This job computes relevance in a script step rather than a
    # `paths:` filter, so the thing to check is not that the file
    # mentions the directory — a comment does that — but that the
    # pattern the step matches changed paths against actually selects
    # one. The pattern is lifted out of the workflow and run.
    pattern=$(sed -n "s/.*grep -qE '\(\^(.*)\)'.*/\1/p" "$BASH_WORKFLOW" | head -1)
    [ -n "$pattern" ]
    run grep -qE "^($pattern)" <<<'tests/arch/boundaries.sh'
    [ "$status" -eq 0 ]
}

@test "the bats job's relevance pattern does not match everything" {
    # A pattern that selects any path at all would satisfy the case
    # above while saying nothing about arch coverage.
    pattern=$(sed -n "s/.*grep -qE '\(\^(.*)\)'.*/\1/p" "$BASH_WORKFLOW" | head -1)
    run ! grep -qE "^($pattern)" <<<'gui/frontend/src/routes/+page.svelte'
}

@test "a sibling token does not read as excluding tests/arch" {
    # `tests/archive` shares a prefix with `tests/arch` and excludes
    # none of it; matching on the prefix alone would fail the exclusion
    # case for a token that leaves these scripts linted.
    tokens=$(printf '%s\n' '      sh_checker_exclude: "tests/archive gui"' |
        parse_exclusions)
    run ! exclusions_unreadable "$tokens"
    subject=tests/arch
    for token in $tokens; do
        case "$subject" in
            "$token" | "$token"/*) return 1 ;;
            *) ;;
        esac
    done
}

@test "the bats job runs when the workflow these tests inspect changes" {
    # `invocation.bats` reads `arch-lint.yml` and asserts things about
    # it, which makes that file a subject under test. If a change
    # touching only it does not select the bats job, gutting the lint
    # workflow — the exact regression the cases above exist to catch —
    # lands with the suite that catches it never having run.
    pattern=$(sed -n "s/.*grep -qE '\(\^(.*)\)'.*/\1/p" "$BASH_WORKFLOW" | head -1)
    [ -n "$pattern" ]
    run grep -qE "^($pattern)" <<<'.github/workflows/arch-lint.yml'
    [ "$status" -eq 0 ]
}

@test "the bats job runs when any workflow changes" {
    # The producer-uniqueness case scans every file under
    # .github/workflows/, which makes the whole directory a subject: a
    # duplicate of the arch lint's check name added in rust.yml must
    # not land while the case that rejects it is skipped as
    # irrelevant.
    pattern=$(sed -n "s/.*grep -qE '\(\^(.*)\)'.*/\1/p" "$BASH_WORKFLOW" | head -1)
    [ -n "$pattern" ]
    run grep -qE "^($pattern)" <<<'.github/workflows/rust.yml'
    [ "$status" -eq 0 ]
}

# Whether a workflow sets up the upstream remote, as a function so a
# gutted spelling runs through exactly the check the live file does.
# Invocation-shaped, like the runner-wiring constraint: `git` has to
# be the command of its line, so an echoed mention does not count.
#
# Deliberately not covered, and said so: the same line as heredoc
# payload, which no regex can tell from a command — that distinction
# needs a shell parser, the interpretation these checks no longer
# attempt. This green means "a line of the setup's exact shape
# exists"; the evasion requires a reviewed workflow edit that spells
# out the mimicry, which is what review is for.
configures_upstream() {
    grep -qE '^[[:space:]]*git remote add upstream([[:space:]]|$)' "$1"
}

@test "the bats job configures the upstream divergence baseline" {
    # `bash_portability.bats` skips both of its cases when no `upstream`
    # remote exists. A fresh CI checkout has none, so without the same
    # setup `arch.yml` performs the ported suite measures nothing and
    # reports success.
    run configures_upstream "$BASH_WORKFLOW"
    [ "$status" -eq 0 ]
}

@test "a step that merely mentions the upstream setup is not configuration" {
    # `echo git remote add upstream ...` prints the command and creates
    # no remote: both portability cases then skip, the suite measures
    # nothing, and a containment match still reports the baseline
    # configured. The command has to be in command position — the same
    # shape the runner-wiring constraint takes.
    mentioned="$BATS_TEST_TMPDIR/mentioned-upstream.yml"
    sed 's/git remote add upstream/echo git remote add upstream/' \
        "$BASH_WORKFLOW" >"$mentioned"
    if cmp -s "$mentioned" "$BASH_WORKFLOW"; then
        echo "sabotage changed nothing"
        return 1
    fi
    run ! configures_upstream "$mentioned"
}

@test "a check pointed at a missing file exits nonzero" {
    # A tool failing inside the pipeline must not leave the check
    # reporting success. `set -e` takes the failing command
    # substitution today; this holds it there.
    run sh "$ARCH_DIR/deferral_record.sh" /definitely/not/here
    [ "$status" -ne 0 ]
}
