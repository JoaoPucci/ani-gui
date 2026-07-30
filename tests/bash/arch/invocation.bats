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

@test "each check resolves the repository when invoked by relative path" {
    for script in deferral_record agents_contract; do
        run bash -c "cd '$ARCH_DIR' && sh ./$script.sh"
        [ "$status" -eq 0 ]
    done
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
    run env __DEFERRAL_RECORD_LIB__=1 ARCH_REPO_ROOT="$probe" \
        sh -c '. "$1/tests/arch/deferral_record.sh"; pwd' _ "$REPO_ROOT"
    [[ "$output" == "$probe"* ]]
}

@test "the shell linter runs for changes under tests/arch" {
    # These are shell too. The workflow filtered on the vendored
    # script's path alone, so a change touching only the arch checks
    # matched nothing and shellcheck never ran — the absence reading
    # as a pass.
    run grep -c 'tests/arch' "$WORKFLOW"
    [ "$status" -eq 0 ]
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
    allowed='^(ARCH_[A-Z0-9_]+|HOME|PATH|TMPDIR|CI)$'
    src=$(sed 's/#.*//' "$ARCH_DIR"/*.sh)
    defaulted=$(printf '%s\n' "$src" | grep -oE '\$\{[A-Z][A-Z0-9_]{2,}:[-=]' |
        sed 's/^\${//; s/:[-=]$//' | sort -u)
    plain=$(printf '%s\n' "$src" | grep -oE '\$\{?[A-Z][A-Z0-9_]{2,}\}?' |
        sed 's/^\$//; s/^{//; s/}$//' | sort -u)
    assigned=$(printf '%s\n' "$src" |
        grep -oE '^[[:space:]]*[A-Z][A-Z0-9_]{2,}=|export[[:space:]]+[A-Z][A-Z0-9_]{2,}' |
        sed 's/^[[:space:]]*//; s/^export[[:space:]]*//; s/=$//' | sort -u)
    if [ -n "$assigned" ]; then
        unowned=$(printf '%s\n' "$plain" | grep -vxF "$assigned" || true)
    else
        unowned=$plain
    fi
    stray=$(printf '%s\n%s\n' "$defaulted" "$unowned" |
        grep -v '^$' | sort -u | grep -vE "$allowed" || true)
    [ -z "$stray" ] || {
        echo "readable from any environment: $stray"
        return 1
    }
}

@test "the bats job runs when a check these tests cover changes" {
    # These tests exercise `tests/arch/*.sh`. The job that runs them
    # decides relevance from the changed paths, so a change to a check
    # and nothing else has to count — otherwise editing the very thing
    # under test skips the suite, and the pull request goes green
    # having run none of it.
    run grep -c 'tests/arch/' "$BASH_WORKFLOW"
    [ "$status" -eq 0 ]
}
