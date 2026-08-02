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

@test "the real checks run from a path containing an apostrophe" {
    # Nothing stops anyone cloning into `~/o'brien/`. A repository
    # root carrying an apostrophe reaches every path expansion a check
    # makes; one unquoted use and the quoting ends where the name
    # does. The subject has to be the actual checks — a trivial script
    # that resolves nothing proves only that the shell can start. The
    # symlink gives the real repository an apostrophe-bearing root:
    # `$0` resolves through it, `pwd` keeps the logical path, and the
    # checks run their real work with the apostrophe in every path
    # they build.
    quoted="$BATS_TEST_TMPDIR/o'brien"
    mkdir -p "$quoted"
    ln -s "$REPO_ROOT" "$quoted/repo"
    for script in deferral_record agents_contract; do
        run sh "$quoted/repo/tests/arch/$script.sh"
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
# Only the paired-quote forms extract: a value whose quote closes on
# the line it opened on, double or single. Everything else — an
# unterminated quote, a block scalar, and the bare plain form, which
# can continue onto the next line with no first-line signal at all —
# passes through untouched, and the surviving key text lands in the
# refusal. The readable set is exactly the spellings whose first line
# provably carries the whole value.
# Which physical line of a workflow file carries the exclusion key,
# as a function so a fixture file runs through the same selection the
# live read runs. A declaration is the line where the key sits at the
# start, modulo indentation — a comment or any other mid-line mention
# is not one and must not shadow the line that is. The key has
# spellings of its own: both quoted forms resolve to the same key the
# bare form declares, a double-quoted key carrying a backslash can be
# that key whatever it looks like, and the explicit form (`? key` /
# `: value`) puts key and value on lines this read cannot join. All
# of them select and count — the quoted forms then refuse in
# parse_exclusions through their surviving key text, and the explicit
# halves each count, so the ambiguity refusal fires. YAML permits
# whitespace before the colon; the pattern tolerates it.
#
# One pattern for selection and count, so the two can never disagree
# about what a declaration is.
EXCLUSION_KEY_RE="^[[:space:]]*(\"sh_checker_exclude\"|'sh_checker_exclude'|sh_checker_exclude|\"[^\"]*\\\\[^\"]*\")[[:space:]]*:|^[[:space:]]*[?:]([[:space:]]|\$)"

select_exclusion_line() {
    grep -E "$EXCLUSION_KEY_RE" "$1" | head -1 || true
}

# How many lines declare the key. One is a list; more than one is two
# exclude lists — a second sh-checker step carries its own — and
# reading the first says nothing about the second, so the live case
# refuses the count rather than picking a winner.
exclusion_declarations() {
    grep -cE "$EXCLUSION_KEY_RE" "$1" || true
}

parse_exclusions() {
    sed "s/.*sh_checker_exclude:[[:space:]]*\"\([^\"]*\)\".*/\1/; t
s/.*sh_checker_exclude:[[:space:]]*'\([^']*\)'.*/\1/; t"
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

# The clean runs shed the same names hostile_env sets — the two lists
# are the same list, name for name. Without this, a caller already
# exporting one of them poisons the baseline: every run sees the name,
# nothing differs, and the sensitivity vanishes. The hostile run needs
# no unsetting; it overrides each name explicitly.
clean_env() {
    env \
        -u REPO_ROOT \
        -u SKIP_NESTED \
        -u GENERIC_GUARD \
        -u ROOT \
        -u DIR \
        -u FILE \
        -u DEBUG -u VERBOSE -u QUIET -u FORCE -u DRY_RUN \
        "$@"
}

# Sensitive when the hostile environment changes what the check prints
# or how it exits. A clean run happens twice first: a check that is not
# reproducible against itself — one that prints durations — differs
# from anything, and for those the exit status is the only stable
# signal.
env_sensitive() {
    _first=$(clean_env sh "$1" 2>&1) && _first_status=0 || _first_status=$?
    _again=$(clean_env sh "$1" 2>&1) || true
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

@test "an exported hostile name cannot poison the clean baseline" {
    # The stray environment the hunt exists to catch can just as
    # easily be the one this suite itself runs under. A caller that
    # already exports a hostile name hands it to the clean runs too:
    # all three runs are then redirected alike, the difference
    # vanishes, and a sensitive check reads as clean. The clean
    # baseline has to shed the hostile names, not merely differ from
    # them.
    export SKIP_NESTED=1
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

# How many lines on stdin declare the arch lint's check name, as a
# function so a fixture spelling runs through exactly the count the
# live scan runs. YAML's bare, double- and single-quoted spellings
# all make the same declaration, so all three count — and each ends
# at its closing delimiter, because a longer name is a different
# check.
# A bare scalar ends at the line's end, an inline comment, or a flow
# terminator; a block-scalar `name:` resolves on the next physical
# line where this count cannot read it, so it counts as a potential
# producer — refusal by over-count, failing closed. The same refusal
# covers the quoted spellings this count cannot read: a double-quoted
# value containing a backslash resolves through escapes it does not
# interpret, and a quote left open on the line continues the scalar
# past where it reads. Single-quoted scalars have no escapes, so only
# their unterminated form is unreadable. It also covers what
# continues or indirects: a bare value that is a strict space-broken
# prefix of the name (or empty) is the first physical line of a
# folded spelling that resolves to the name, and a value opening with
# an anchor, alias or tag resolves somewhere a line count never
# looks. The key side tolerates whitespace before the colon, which
# YAML strips.
count_arch_lint_names() {
    # The key side accepts the spellings that resolve to `name`: the
    # bare form, both quoted forms, and — refusal by over-count — any
    # double-quoted key carrying a backslash, since escapes can spell
    # the key however they like. The trailing alternative counts
    # explicit-form lines (`? key` / `: value`), where key and value
    # never share a physical line; counting both halves over-counts
    # one declaration, the failing-closed direction.
    local name_re='Arch Shellcheck \+ Shfmt'
    grep -cE "(\"name\"|'name'|name|\"[^\"]*\\\\[^\"]*\")[[:space:]]*:[[:space:]]*(\"$name_re\"|'$name_re'|${name_re}[[:space:]]*([]#,}].*)?\$|[>|&*!]|\"[^\"]*\\\\[^\"]*\"|\"[^\"]*\$|'[^']*\$|(Arch( Shellcheck( \+)?)?)?[[:space:]]*\$)|^[[:space:]]*[?:]([[:space:]]|\$)" || true
}

# Every workflow file in a directory, as a function so a fixture
# directory runs through the same enumeration the live scan runs.
# GitHub loads both extensions, so both are read.
scan_workflows() {
    cat "$1"/*.yml "$1"/*.yaml 2>/dev/null || true
}

@test "exactly one workflow reports the arch lint check name" {
    # The green has to be attributable. `Shellcheck + Shfmt` is a name
    # three workflows report, two of them succeeding without reading
    # these files, and branch protection accepts the first success
    # under a required name — so the arch lint owns a name nothing
    # else answers for.
    count=$(scan_workflows "$REPO_ROOT/.github/workflows" | count_arch_lint_names)
    [ "$count" -eq 1 ]
}

@test "a quoted spelling of the check name counts as a producer" {
    # YAML accepts `name: "Arch Shellcheck + Shfmt"` and the single-
    # quoted form as the same declaration. A count that reads only the
    # bare spelling stays at one while a quoted second producer
    # answers for the required check name — the exact ambiguity the
    # uniqueness case exists to reject.
    quoted_count=$(printf '%s\n' '    name: "Arch Shellcheck + Shfmt"' |
        count_arch_lint_names)
    single_count=$(printf '%s\n' "    name: 'Arch Shellcheck + Shfmt'" |
        count_arch_lint_names)
    [ "$quoted_count" -eq 1 ]
    [ "$single_count" -eq 1 ]
}

@test "a block-scalar name declaration is not silently missed" {
    # `name: >-` resolves to the job name on the next physical line,
    # where a single-line count cannot read it — a second producer
    # hides behind the fold. Counted as a potential producer, any
    # block-scalar job name fails the uniqueness case loudly instead:
    # refusal by over-count, the failing-closed direction.
    block_count=$(printf 'jobs:\n  a:\n    name: >-\n      Arch Shellcheck + Shfmt\n' |
        count_arch_lint_names)
    [ "$block_count" -ge 1 ]
}

@test "a commented bare name counts as a producer" {
    # A bare scalar ends at the line's end or at an inline comment;
    # `name: Arch Shellcheck + Shfmt # required` resolves to exactly
    # the required name. The comment completes the bare form's
    # delimiter set.
    comment_count=$(printf '%s\n' '    name: Arch Shellcheck + Shfmt # required job' |
        count_arch_lint_names)
    [ "$comment_count" -eq 1 ]
}

@test "a flow-style name counts as a producer" {
    # Flow style puts the whole job on one line: in
    # `jobs: {stub: {name: Arch Shellcheck + Shfmt}}` the bare scalar
    # ends at the closing brace, not at the line's end. The flow
    # terminators `}`, `,` and `]` complete the bare form's delimiter
    # set alongside the comment. The count does not track flow
    # context, so a block-context scalar continuing with one of these
    # characters spells a longer name yet still counts — an over-count
    # that fails the uniqueness case loudly, the failing-closed
    # direction.
    flow_count=$(printf '%s\n' '    jobs: {stub: {name: Arch Shellcheck + Shfmt}}' |
        count_arch_lint_names)
    [ "$flow_count" -eq 1 ]
}

@test "an unreadable quoted spelling is not silently missed" {
    # A double-quoted scalar can spell the name through escapes —
    # `\u002b` resolves to `+` — and a
    # quote left open on the line continues the scalar where a line
    # count cannot follow. Neither spelling is readable here, so both
    # count as potential producers: refusal by over-count, the same
    # failing-closed arm the block scalar takes. Single-quoted scalars
    # have no escapes — a backslash there is a literal, a different
    # name — so only the unterminated form of that spelling is
    # unreadable.
    escaped_count=$(printf '%s %s\n' '    name:' '"Arch Shellcheck \u002b Shfmt"' |
        count_arch_lint_names)
    open_dq_count=$(printf '%s\n' '    name: "Arch Shellcheck + Shfmt' |
        count_arch_lint_names)
    open_sq_count=$(printf '%s\n' "    name: 'Arch Shellcheck + Shfmt" |
        count_arch_lint_names)
    [ "$escaped_count" -ge 1 ]
    [ "$open_dq_count" -ge 1 ]
    [ "$open_sq_count" -ge 1 ]
}

@test "a bare exclusion list is refused, not scanned" {
    # A plain scalar continues onto an indented next line with nothing
    # on its first line to say so: the fragment before the break is a
    # valid-looking token list missing exactly the entry that
    # mattered. The readable set is the quoted single-line forms.
    probe=$(printf '%s\n' '      sh_checker_exclude: ani-cli' |
        parse_exclusions)
    exclusions_unreadable "$probe"
}

@test "a .yaml workflow enters the producer scan" {
    # GitHub loads workflows with either extension. A scan reading
    # only *.yml never sees a duplicate.yaml declaring the name, and
    # the uniqueness case certifies a name two jobs answer for while
    # the file that breaks it sits beside the ones it read.
    yaml_dir="$BATS_TEST_TMPDIR/yaml-scan"
    mkdir "$yaml_dir"
    printf 'jobs:\n  a:\n    name: Arch Shellcheck + Shfmt\n' >"$yaml_dir/a.yml"
    printf 'jobs:\n  b:\n    name: Arch Shellcheck + Shfmt\n' >"$yaml_dir/b.yaml"
    yaml_count=$(scan_workflows "$yaml_dir" | count_arch_lint_names)
    [ "$yaml_count" -eq 2 ]
}

@test "the bats job runs when a .yaml workflow changes" {
    # The scan reads the whole directory, so a workflow with the
    # longer extension is a subject too: a duplicate producer in
    # duplicate.yaml must not land while the case that rejects it is
    # skipped as irrelevant.
    pattern=$(sed -n "s/.*grep -qE '\(\^(.*)\)'.*/\1/p" "$BASH_WORKFLOW" | head -1)
    [ -n "$pattern" ]
    run grep -qE "^($pattern)" <<<'.github/workflows/duplicate.yaml'
    [ "$status" -eq 0 ]
}

@test "a longer name does not count as a producer" {
    # `Arch Shellcheck + Shfmt (legacy)` is a different check — it
    # cannot answer for the required name — but a prefix-only count
    # includes it, and the uniqueness case then fails with two
    # producers while only one job holds the name that matters. The
    # count has to stop at the name's closing delimiter.
    suffix_count=$(printf '%s\n' '    name: Arch Shellcheck + Shfmt (legacy)' |
        count_arch_lint_names)
    suffix_quoted=$(printf '%s\n' '    name: "Arch Shellcheck + Shfmt (legacy)"' |
        count_arch_lint_names)
    [ "$suffix_count" -eq 0 ]
    [ "$suffix_quoted" -eq 0 ]
}

@test "a folded, indirect or empty name spelling is not silently missed" {
    # A plain scalar continues onto a more-indented next line and
    # folds with a space: `name: Arch Shellcheck` over `+ Shfmt`
    # resolves to exactly the required name while neither physical
    # line spells it. Every folded spelling starts with a strict
    # space-broken prefix of the name — a finite set for a fixed
    # literal — so a bare prefix line counts as a potential producer:
    # refusal by over-count, failing closed. `name:` with an empty
    # value is the zero-length prefix of the same spelling, and a
    # value opening with an anchor, alias or tag resolves through
    # indirection a line count does not follow, so those are refused
    # the same way.
    folded_count=$(printf 'jobs:\n  a:\n    name: Arch Shellcheck\n      + Shfmt\n' |
        count_arch_lint_names)
    empty_count=$(printf '%s\n' '    name:' | count_arch_lint_names)
    alias_count=$(printf '%s\n' '    name: *shared-name' | count_arch_lint_names)
    tag_count=$(printf '%s\n' '    name: !!str Arch Shellcheck + Shfmt' |
        count_arch_lint_names)
    [ "$folded_count" -ge 1 ]
    [ "$empty_count" -ge 1 ]
    [ "$alias_count" -ge 1 ]
    [ "$tag_count" -ge 1 ]
}

@test "a spaced key colon still counts as a producer" {
    # YAML permits whitespace between a key and its colon:
    # `name : Arch Shellcheck + Shfmt` declares the same job name the
    # unspaced spelling declares. A pattern requiring the colon to
    # touch the key reads the spaced form as no declaration at all.
    spaced_count=$(printf '%s\n' '    name : Arch Shellcheck + Shfmt' |
        count_arch_lint_names)
    [ "$spaced_count" -eq 1 ]
}

@test "a quoted, escaped or explicit key spelling is not silently missed" {
    # The key itself has spellings too. A quoted key resolves to the
    # same mapping key the bare form declares; a double-quoted key
    # containing a backslash resolves through escapes this count does
    # not interpret, so it could be `name` whatever it looks like;
    # and the explicit form puts the key on a `?` line with the value
    # on a `:` line beneath it, where no line carries both. The
    # readable spellings count as the bare key does, and the
    # unreadable ones count as potential producers — refusal by
    # over-count, failing closed.
    dq_key_count=$(printf '%s\n' '    "name": Arch Shellcheck + Shfmt' |
        count_arch_lint_names)
    sq_key_count=$(printf '%s\n' "    'name': Arch Shellcheck + Shfmt" |
        count_arch_lint_names)
    esc_key_count=$(printf '%s\n' '    "na\u006de": Arch Shellcheck + Shfmt' |
        count_arch_lint_names)
    explicit_key_count=$(printf '    ? name\n    : Arch Shellcheck + Shfmt\n' |
        count_arch_lint_names)
    [ "$dq_key_count" -eq 1 ]
    [ "$sq_key_count" -eq 1 ]
    [ "$esc_key_count" -ge 1 ]
    [ "$explicit_key_count" -ge 1 ]
}

@test "a quoted or explicit exclusion key is not silently missed" {
    # "sh_checker_exclude": "ani-cli tests/arch" is a valid action
    # input resolving to the same key the bare spelling declares, but
    # a selection reading only the bare form never sees it: the token
    # list comes back empty, empty reads as excluding nothing, and
    # the case above certifies scripts the action excludes. Seen, the
    # quoted-key line refuses in the parse through its surviving key
    # text — so the fix is selection, and the refusal already waits
    # behind it. The explicit form counts both its lines, so the
    # ambiguity refusal fires on it.
    quoted_key_excl="$BATS_TEST_TMPDIR/quoted-key-excl.yml"
    printf '%s\n' '          "sh_checker_exclude": "ani-cli tests/arch"' \
        >"$quoted_key_excl"
    quoted_key_count=$(exclusion_declarations "$quoted_key_excl")
    quoted_key_tokens=$(select_exclusion_line "$quoted_key_excl" | parse_exclusions)
    explicit_key_excl="$BATS_TEST_TMPDIR/explicit-key-excl.yml"
    printf '          ? sh_checker_exclude\n          : "ani-cli tests/arch"\n' \
        >"$explicit_key_excl"
    explicit_excl_count=$(exclusion_declarations "$explicit_key_excl")
    [ "$quoted_key_count" -eq 1 ]
    exclusions_unreadable "$quoted_key_tokens"
    [ "${explicit_excl_count:-0}" -ge 1 ]
}

@test "a commented mention does not shadow the declaration beneath it" {
    # A comment mentioning the key is not a declaration. Selected as
    # one, it shadows the live line beneath it: the comment's tokens
    # parse cleanly, the declaration that actually configures the
    # action is never read, and the case above reports the scripts
    # linted while the live list excludes them.
    shadow_input="$BATS_TEST_TMPDIR/comment-shadow.yml"
    printf '%s\n' \
        '          # sh_checker_exclude: "ani-cli"' \
        '          sh_checker_exclude: "ani-cli tests/arch"' \
        >"$shadow_input"
    shadow_tokens=$(select_exclusion_line "$shadow_input" | parse_exclusions)
    shadow_hit=0
    subject=tests/arch
    for token in $shadow_tokens; do
        case "$subject" in
            "$token" | "$token"/*) shadow_hit=1 ;;
            *) ;;
        esac
    done
    [ "$shadow_hit" -eq 1 ]
}

@test "a second exclusion declaration is counted, not skipped" {
    # Two declarations are two exclude lists — a second sh-checker
    # step carries its own — and reading the first says nothing about
    # the second, which is exactly where an exclusion of these
    # scripts would hide. The count is the fact the live case refuses
    # on; more than one declaration is ambiguity, not a list.
    ambiguous_input="$BATS_TEST_TMPDIR/ambiguous-excl.yml"
    printf '%s\n' \
        '          sh_checker_exclude: "ani-cli"' \
        '          sh_checker_exclude: "tests/arch"' \
        >"$ambiguous_input"
    ambiguous_count=$(exclusion_declarations "$ambiguous_input" 2>/dev/null || true)
    [ "${ambiguous_count:-0}" -eq 2 ]
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
    # More than one declaration is the same situation reached another
    # way: the list this read did not pick may be the one that
    # matters.
    declaration_count=$(exclusion_declarations "$WORKFLOW")
    [ "$declaration_count" -le 1 ]
    line=$(select_exclusion_line "$WORKFLOW")
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
