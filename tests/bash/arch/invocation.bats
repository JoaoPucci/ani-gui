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
    WORKFLOW="$REPO_ROOT/.github/workflows/ani-cli.yml"
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

# Whether a workflow's `paths:` filter carries a list entry covering a
# directory. A bare search of the file answers yes to a comment that
# merely names the directory, so the entry can be deleted and the
# assertion stays green — which is the failure this whole file is
# about. Anchored to the list item, and scoped to the trigger block so
# prose further down cannot stand in for wiring.
filter_covers() {
    # The path has to end on a boundary, or a sibling that merely
    # shares the prefix — `tests/archive` for `tests/arch` — satisfies
    # a request it covers none of.
    sed -n '/^  pull_request:/,/^jobs:/p' "$1" |
        grep -qE "^[[:space:]]*-[[:space:]]*[\"']?$2([/\"'[:space:]]|$)"
}

# Names the arch scripts read from the environment, over whatever
# source is on stdin. A function rather than a pipeline inline in the
# live assertion, so a fixture runs through the identical code —
# asserting only against the real scripts means the day it stops
# detecting anything, it reports ok.

# The part of a script the shell would actually expand. Two things are
# dropped, for the same reason: the shell never reads them as code, so
# a name appearing there is not a read of anything.
#
# Comments. A `#` opens one only where a word could start — at the
# beginning of a line or after a blank — and only outside quotes.
# `sed 's/#.*//'` honours neither rule, so `"#${GENERIC_GUARD-}"` lost
# its name to a hash that was never a comment.
#
# Single-quoted spans. `$name` inside them is literal, by definition of
# single quotes. Embedded awk is where this bites: `boundaries.sh`
# carries `for (i = 3; ...) rest = rest ":" $i`, where `$i` is awk's
# field reference, and an audit that reads it as shell reports `i` as
# an environment variable the checks depend on. Double quotes are kept,
# because the shell does expand inside them.
#
# Quote state carries across lines, because the strings that matter
# here do. `boundaries.sh` holds a multi-line awk program in single
# quotes; resetting per line makes its interior read as ordinary shell
# from the second line on, which is exactly where its `$i` sits.
expandable_text() {
    awk '
        BEGIN { sq = sprintf("%c", 39); q = "" }
        {
            out = ""; prev = ""; i = 1; n = length($0)
            while (i <= n) {
                c = substr($0, i, 1)
                if (q == "") {
                    if (c == "\\") {
                        out = out c substr($0, i + 1, 1); prev = "x"; i += 2; continue
                    }
                    if (c == "#" && (out == "" || prev == " " || prev == "\t")) break
                    if (c == sq) { q = c; prev = c; i++; continue }
                    if (c == "\"") q = c
                } else if (q == sq) {
                    if (c == sq) q = ""
                    prev = c; i++; continue
                } else if (c == "\\") {
                    out = out c substr($0, i + 1, 1); prev = "x"; i += 2; continue
                } else if (c == q) {
                    q = ""
                }
                out = out c; prev = c; i++
            }
            print out
        }
    ' "$@"
}

ambient_stray_names() {
    # Case is not part of the rule. `skip_nested` is as legal an
    # environment variable as `SKIP_NESTED`, so the alphabet matched
    # below is the alphabet the shell accepts.
    #
    # The allowlist is deliberately not widened to absorb that. Every
    # lowercase local in these scripts is assigned somewhere in them,
    # which is what makes it a local, and the assignment pass is what
    # accounts for it. Exempting lower case by name would take back
    # the whole of what widening the alphabet buys.
    local allowed='^(ARCH_[A-Za-z0-9_]+|HOME|PATH|TMPDIR|CI)$'
    local src defaulted plain assigned unowned
    src=$(expandable_text)
    # Every expansion whose result can come from outside: the four
    # POSIX operators, each with and without the colon. The colon only
    # decides whether an empty value counts as unset — it says nothing
    # about where the value came from.
    defaulted=$(printf '%s\n' "$src" |
        grep -oE '\$\{[A-Za-z_][A-Za-z0-9_]*:?[-=?+]' |
        sed 's/^\${//; s/[-:=?+].*$//' | sort -u)
    plain=$(printf '%s\n' "$src" | grep -oE '\$\{?[A-Za-z_][A-Za-z0-9_]*\}?' |
        sed 's/^\$//; s/^{//; s/}$//' | sort -u)
    # Four ways the suite owns a name: a plain assignment, an export
    # carrying a value, a `for` that binds it, and a `read` that binds
    # it. Omitting any of the binding forms turns an ordinary local
    # into a reported finding, and this audit gates every change.
    assigned=$(printf '%s\n' "$src" |
        grep -oE '^[[:space:]]*[A-Za-z_][A-Za-z0-9_]*=|^[[:space:]]*for[[:space:]]+[A-Za-z_][A-Za-z0-9_]*|export[[:space:]]+[A-Za-z_][A-Za-z0-9_]*=' |
        sed 's/^[[:space:]]*//; s/^for[[:space:]]*//; s/^export[[:space:]]*//; s/=$//' |
        sort -u)
    # `read` takes its flags first, so the bound name is the last token
    # of the match. Multiple names on one `read` would need more than
    # this; nothing here spells one.
    assigned=$(printf '%s\n%s\n' "$assigned" "$(printf '%s\n' "$src" |
        grep -oE '(^|[[:space:];|&])read[[:space:]]+(-[A-Za-z]+[[:space:]]+)*[A-Za-z_][A-Za-z0-9_]*' |
        sed 's/.*[[:space:]]//')" | grep -v '^$' | sort -u)
    if [ -n "$assigned" ]; then
        unowned=$(printf '%s\n' "$plain" | grep -vxF "$assigned" || true)
    else
        unowned=$plain
    fi
    printf '%s\n%s\n' "$defaulted" "$unowned" |
        grep -v '^$' | sort -u | grep -vE "$allowed" || true
}

@test "the shell linter runs for changes under tests/arch" {
    # These are shell too. The workflow filtered on the vendored
    # script's path alone, so a change touching only the arch checks
    # matched nothing and shellcheck never ran — the absence reading
    # as a pass.
    filter_covers "$WORKFLOW" 'tests/arch'
}

@test "a comment naming the directory is not path wiring" {
    # The fixture carries the explanatory comment and no list entry,
    # which is what deleting the entry leaves behind. A search of the
    # whole file cannot tell the two apart.
    fixture="$BATS_TEST_TMPDIR/commented.yml"
    cat >"$fixture" <<'YAML'
on:
  pull_request:
    paths:
      - "**ani-cli"
jobs:
  lint:
    # `tests/arch` is ordinary POSIX sh and is linted.
    runs-on: ubuntu-latest
YAML
    run ! filter_covers "$fixture" 'tests/arch'
}

@test "the linter does not exclude tests/arch from checking" {
    # Starting the job is not the same as inspecting anything. The
    # action takes its own exclude list, and a bare `tests` there skips
    # these files after the workflow has started for them.
    excluded=$(grep 'sh_checker_exclude' "$WORKFLOW" | head -1 |
        sed 's/.*sh_checker_exclude:[[:space:]]*"//; s/".*//')
    subject=tests/arch
    for token in $excluded; do
        case "$subject" in
            "$token" | "$token"/*) return 1 ;;
            *) ;;
        esac
    done
}

@test "every ambient variable the checks read is namespaced" {
    # A generic name is an input from whatever shell the checks happen
    # to run in, whether or not anyone meant it as one — which is how
    # `REPO_ROOT` and `SKIP_NESTED` each silently changed what a check
    # did, and each was found by a reviewer rather than by a run.
    #
    # Reading with a default says outright that the name may be
    # absent, so it may arrive from outside. Reading without one is
    # ambient only when nothing assigns it, since otherwise it is an
    # ordinary local. Comments are stripped, or this paragraph would
    # flag itself.
    stray=$(expandable_text "$ARCH_DIR"/*.sh | ambient_stray_names)
    [ -z "$stray" ] || {
        echo "readable from any environment: $stray"
        return 1
    }
}

@test "a single-character name is still ambient" {
    # One uppercase letter is a valid environment variable, so the
    # shortest name the audit sees has to be the shortest the shell
    # accepts.
    run ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/single-char-name.sh"
    [[ "$output" == *X* ]]
}

@test "a two-character name is still ambient" {
    # `[A-Z][A-Z0-9_]*` needs three characters, so a legal
    # two-character environment name is invisible to all three passes
    # at once — not just the default-expansion one. `CI` sits on the
    # allowlist and is itself two characters, so under that pattern the
    # entry never did anything.
    run ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/short-name.sh"
    [[ "$output" == *NO* ]]
}

@test "a colonless default expansion is still ambient" {
    # `${VAR-}` and `${VAR=x}` are POSIX and mean the same thing as
    # their colon spellings: the value may arrive from outside. The
    # gap only opens when the name is also assigned, which is what a
    # self-invocation guard does — the assignment closes the
    # read-but-never-assigned path, leaving the default-expansion path
    # as the only thing that can see it.
    #
    # The fixture is a file outside tests/arch because the audit scans
    # that directory; spelled inline, the test data is reported as a
    # real finding.
    run ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/colonless-default.sh"
    [[ "$output" == *SKIP_NESTED* ]]
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

@test "a sibling path does not satisfy the filter" {
    # `tests/archive/**.sh` shares a prefix with `tests/arch` and covers
    # none of it. Anchoring only the start of the list item accepts it,
    # so the wiring can be repointed at an unrelated directory while
    # the case above stays green.
    fixture="$BATS_TEST_TMPDIR/sibling.yml"
    cat >"$fixture" <<'YAML'
on:
  pull_request:
    paths:
      - "tests/archive/**.sh"
jobs:
  lint:
    runs-on: ubuntu-latest
YAML
    run ! filter_covers "$fixture" 'tests/arch'
}

@test "the bats job runs when the workflow these tests inspect changes" {
    # `invocation.bats` reads `ani-cli.yml` and asserts things about it,
    # which makes that file a subject under test. If a change touching
    # only it does not select the bats job, deleting the path entry —
    # the exact regression the case above exists to catch — lands with
    # the suite that catches it never having run.
    pattern=$(sed -n "s/.*grep -qE '\(\^(.*)\)'.*/\1/p" "$BASH_WORKFLOW" | head -1)
    [ -n "$pattern" ]
    run grep -qE "^($pattern)" <<<'.github/workflows/ani-cli.yml'
    [ "$status" -eq 0 ]
}

@test "the bats job configures the upstream divergence baseline" {
    # `bash_portability.bats` skips both of its cases when no `upstream`
    # remote exists. A fresh CI checkout has none, so without the same
    # setup `arch.yml` performs the ported suite measures nothing and
    # reports success.
    run grep -qE 'remote add upstream' "$BASH_WORKFLOW"
    [ "$status" -eq 0 ]
}

@test "a check pointed at a missing file exits nonzero" {
    # A tool failing inside the pipeline must not leave the check
    # reporting success. `set -e` takes the failing command
    # substitution today; this holds it there.
    run sh "$ARCH_DIR/deferral_record.sh" /definitely/not/here
    [ "$status" -ne 0 ]
}

@test "a quoted hash does not hide the rest of the line" {
    # `#` opens a comment only where a word could start and only
    # outside quotes, so `"#${GENERIC_GUARD-}"` is an ordinary
    # comparison. Stripping from the first hash on the line swallows
    # the rest of it and the audit reports ok about a name it never
    # saw.
    run ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/quoted-hash.sh"
    [[ "$output" == *GENERIC_GUARD* ]]
}

@test "a valueless export does not claim ownership" {
    # `export NAME` assigns nothing. It marks the name for the
    # environment and whatever the calling shell put there survives,
    # so the read is exactly as ambient as it would be with no export
    # at all.
    run ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/bare-export.sh"
    [[ "$output" == *GENERIC_GUARD* ]]
}

@test "a lowercase name is still ambient" {
    # Shell names are not required to be upper case. `skip_nested` is
    # as legal an environment variable as `SKIP_NESTED` and an export
    # reaches the read the same way; the shouting is a convention, not
    # a rule the shell enforces. Patterns anchored on `[A-Z]` see none
    # of it.
    run ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/lowercase-name.sh"
    [[ "$output" == *skip_nested* ]]
}

@test "an uppercase loop variable is a local, not an ambient input" {
    # `for SCRIPT in ...` assigns SCRIPT, so nothing about it comes
    # from the calling shell. Recognising `NAME=` and `export NAME` but
    # not `for NAME` turns a valid local into a reported finding, and
    # this suite gates every change.
    run ambient_stray_names <"$REPO_ROOT/tests/fixtures/arch/uppercase-loop-var.sh"
    [[ "$output" != *SCRIPT* ]]
}
