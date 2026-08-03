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
# The trigger key has spellings — quoted forms and key-colon spacing
# resolve to the same key — so the range accepts them, and its end is
# any following top-level key whatever its spelling. A block that
# cannot be located at all reads as conditional: an empty range
# contains no `paths` for the wrong reason, and certifying from it is
# the failing-open direction.
unconditional() {
    [ -f "$1" ] || return 1
    _trigger=$(sed -nE "/^(\"on\"|'on'|on)[[:space:]]*:/,/^[^[:space:]#]/p" "$1")
    [ -n "$_trigger" ] || return 1
    # A key spelling inside the block that this reading cannot
    # resolve — an escape-bearing quoted key, an alias or anchor
    # token, the explicit form — reads as a filter being hidden, not
    # as its absence: the block is small and owned by this
    # repository, and every honest spelling it needs is readable.
    if printf '%s\n' "$_trigger" |
        grep -qE "\"[^\"]*\\\\[^\"]*\"[[:space:]]*:|^[[:space:]]*[*&][^:]*:|^[[:space:]]*[?:]([[:space:]]|\$)"; then
        return 1
    fi
    ! printf '%s\n' "$_trigger" | grep -q 'paths'
}

# The regex naming the action, shared by the file-wide count and the
# job-scoped one so the two can never disagree about what an
# invocation is. Anchored to a step-shaped line: `uses:` at the start
# of its line, optionally behind a list dash — a comment or any other
# mid-line mention is text about the action, not an invocation of it.
LINT_ACTION_RE="^[[:space:]]*(-[[:space:]]+)?uses[[:space:]]*:[[:space:]]*['\"]?luizm/action-sh-checker"

# How many step lines invoke the sh-checker action. Carrying the
# required name is not linting: the job has to reach the action for
# anything to be inspected, and the count is a syntactic constraint —
# a step-shaped `uses:` line naming the action either exists or does
# not.
lint_action_uses() {
    grep -cE "$LINT_ACTION_RE" "$1" || true
}

# The remainder of the job that carries the required name: the lines
# from its bare name declaration to the next line at job-id
# indentation, or the end of the file. The same range family the
# trigger reading uses — an indentation constraint, not a parse. Two
# stated bounds: the segment starts at the first line spelling the
# bare name, so a job declaring its name after its steps reads as
# empty and refuses; a quoted or otherwise indirect name spelling is
# not found and refuses the same way. Both directions fail closed —
# the remedy is spelling the job the way the live workflow spells it.
required_job_segment() {
    awk -v name="name: Arch Shellcheck + Shfmt" '
        found && /^  [^ ]/ { exit }
        found { print }
        index($0, name) { found = 1 }
    ' "$1" 2>/dev/null
}

# Whether the workflow actually lints where branch protection looks:
# within the job carrying the required name, exactly one invocation
# of the action and exactly one exclusion declaration. File-wide
# counts certify a workflow whose name-holding job inspects nothing
# while a second job lints somewhere branch protection never reads.
# Zero of either refuses as a job that starts, succeeds and inspects
# nothing; more than one is ambiguity the reads below already refuse.
lint_step_present() {
    _segment=$(required_job_segment "$1")
    [ -n "$_segment" ] || return 1
    [ "$(printf '%s\n' "$_segment" | grep -cE "$LINT_ACTION_RE" || true)" -eq 1 ] &&
        [ "$(printf '%s\n' "$_segment" | grep -cE "$EXCLUSION_KEY_RE" || true)" -eq 1 ]
}

# The relevance pattern is read from the conditional that sets
# relevant=true — the `if grep` fed by "$changed" — never from the
# first grep that happens to appear in the file: a diagnostic grep is
# not the predicate. More than one governing-shaped line is ambiguity
# and refuses, so the probes fail rather than trust a guess.
relevance_pattern() {
    _governing=$(grep -F 'if grep -qE' "$1" | grep -F '<<<"$changed"' || true)
    [ "$(printf '%s\n' "$_governing" | grep -c .)" -eq 1 ] || return 1
    printf '%s\n' "$_governing" | sed -n "s/.*grep -qE '\(\^(.*)\)'.*/\1/p"
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
# The authority behind every text reading here: parse the workflows
# with a real parser and certify the RESOLVED structure, where
# quoting, escapes, folding, flow style, aliases and merge keys have
# already collapsed to the values GitHub sees. The text arms remain
# as the conservative belt; a spelling none of them names lands here,
# because resolution does not read spellings at all. Certified:
# exactly one job resolves to the required name, its trigger carries
# no paths filter at any depth, the job invokes exactly one step
# whose action repository is exactly luizm/action-sh-checker, and
# that step declares a literal exclude list whose tokens do not cover
# tests/arch. Refused rather than certified: unparseable files,
# expression-valued names or exclusions, a missing parser. Stated
# boundaries: PyYAML resolves YAML 1.1, where a bare `on` key reads
# as boolean True — handled — and GitHub parses its own dialect at
# the edges; a divergence surfaces as a refusal or a wrong producer
# count, never a silent pass.
# The interpreter that runs the certification: PATH's python3 when it
# can import yaml — a virtualenv that provides PyYAML is the
# environment's choice — otherwise /usr/bin/python3, where the Debian
# package installs it. A shim without the module no longer answers
# for the provisioned interpreter beside it; when neither can import
# yaml, the missing-parser refusal stands.
resolution_python() {
    _py=""
    for _py in python3 /usr/bin/python3; do
        if "$_py" -c 'import yaml' 2>/dev/null; then
            printf '%s\n' "$_py"
            return 0
        fi
    done
    return 1
}

certified_by_resolution() {
    _resolution_py=$(resolution_python) || {
        printf 'resolution layer unavailable: no python3 with PyYAML\n' >&2
        return 1
    }
    "$_resolution_py" - "$1" "Arch Shellcheck + Shfmt" <<'PYCERT'
import os
import sys

try:
    import yaml
except Exception:
    print("resolution layer unavailable: PyYAML missing", file=sys.stderr)
    sys.exit(1)

wfdir, required = sys.argv[1], sys.argv[2]
SUBJECT = "tests/arch"
ACTION = "luizm/action-sh-checker"
producers = []
problems = []


def gated(node):
    if isinstance(node, dict):
        return any(
            key in ("paths", "paths-ignore") or gated(value)
            for key, value in node.items()
        )
    if isinstance(node, list):
        return any(gated(value) for value in node)
    return False


# The directory is a literal path, never pattern syntax: a checkout
# under a name carrying a glob metacharacter enumerates like any
# other.
try:
    entries = sorted(os.listdir(wfdir))
except OSError as exc:
    entries = []
    problems.append(f"{wfdir}: unreadable: {exc}")
paths = [
    os.path.join(wfdir, entry)
    for entry in entries
    if entry.endswith((".yml", ".yaml"))
]
if not paths and not problems:
    problems.append("no workflows to read")
for path in paths:
    try:
        with open(path, encoding="utf-8") as handle:
            docs = list(yaml.safe_load_all(handle))
    except Exception as exc:
        problems.append(f"{path}: unparseable: {exc}")
        continue
    for doc in docs:
        if not isinstance(doc, dict):
            continue
        jobs = doc.get("jobs")
        if jobs is None:
            continue
        if not isinstance(jobs, dict):
            problems.append(f"{path}: jobs is not a mapping")
            continue
        for jid, job in jobs.items():
            if not isinstance(job, dict):
                continue
            name = job.get("name")
            if isinstance(name, str) and "${{" in name:
                problems.append(f"{path}: job {jid}: expression-valued name refused")
                continue
            if name != required:
                continue
            producers.append((path, doc, jid, job))

if len(producers) != 1:
    problems.append(f"{len(producers)} resolved producers of the required name, not 1")
def pr_unrestricted(trig):
    # Branch protection gates pull requests, so the producer must
    # report on them: `pull_request` as the whole trigger, a member
    # of its list, or a key with a bare value. A pull_request carrying
    # any configuration — types, branches, paths — restricts when the
    # check reports, and a restriction this reading cannot vouch for
    # refuses.
    if trig == "pull_request":
        return True
    if isinstance(trig, list):
        return "pull_request" in trig
    if isinstance(trig, dict):
        return "pull_request" in trig and trig["pull_request"] is None
    return False


for path, doc, jid, job in producers:
    trigger = doc.get("on", doc.get(True))
    if trigger is None:
        problems.append(f"{path}: producer has no trigger")
    if gated(trigger):
        problems.append(f"{path}: producer trigger is path-filtered")
    if not pr_unrestricted(trigger):
        problems.append(f"{path}: trigger lacks an unrestricted pull_request event")
    # A conditional or failure-tolerant job cannot gate anything,
    # whatever its steps do.
    if "if" in job:
        problems.append(f"{path}: job {jid} is conditional")
    if job.get("continue-on-error") not in (None, False):
        problems.append(f"{path}: job {jid} tolerates failure")
    steps = job.get("steps")
    if not isinstance(steps, list):
        problems.append(f"{path}: job {jid} has no steps")
        steps = []
    lint = []
    for step in steps:
        if not isinstance(step, dict):
            continue
        uses = step.get("uses")
        if isinstance(uses, str) and uses.split("@", 1)[0] == ACTION:
            lint.append(step)
    if len(lint) != 1:
        problems.append(f"{path}: job {jid} invokes the lint action {len(lint)} times, not 1")
        continue
    # A step that can be skipped or whose failure is ignored counts
    # as no lint at all: the job then succeeds and satisfies branch
    # protection while inspecting nothing.
    if "if" in lint[0]:
        problems.append(f"{path}: the lint step is conditional")
    if lint[0].get("continue-on-error") not in (None, False):
        problems.append(f"{path}: the lint step tolerates failure")
    inputs = lint[0].get("with")
    if not isinstance(inputs, dict):
        problems.append(f"{path}: the lint step declares no inputs")
        continue
    excl = inputs.get("sh_checker_exclude", "")
    if not isinstance(excl, str) or "${{" in excl:
        problems.append(f"{path}: exclusion is not a readable literal")
        continue
    for token in excl.split():
        if SUBJECT == token or SUBJECT.startswith(token + "/"):
            problems.append(f"{path}: exclusion covers {SUBJECT} via {token!r}")

for problem in problems:
    print(problem, file=sys.stderr)
sys.exit(1 if problems else 0)
PYCERT
}

# The alias/anchor arm selects any line opening with `*` or `&` that
# carries a colon: `*k: "list"` resolves to a key this read cannot
# name, so it must be seen and then refused, while `&a key: value`
# anchors a key that parses normally once selected.
EXCLUSION_KEY_RE="^[[:space:]]*(\"sh_checker_exclude\"|'sh_checker_exclude'|sh_checker_exclude|\"[^\"]*\\\\[^\"]*\")[[:space:]]*:|^[[:space:]]*[?:]([[:space:]]|\$)|^[[:space:]]*[*&][^:]*:"

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
    # The leading strip means an unextractable line reaches the
    # refusal with its first real character exposed — an alias or
    # anchor opener is then caught by the same arms that refuse those
    # openers in values. The strip's own substitution would satisfy
    # the first `t` and skip the second extraction, so a branch to
    # the next line clears the flag before the extractions run.
    sed "s/^[[:space:]]*//
t clear
: clear
s/.*sh_checker_exclude:[[:space:]]*\"\([^\"]*\)\".*/\1/; t
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
    # shellcheck disable=SC2016 # the unexpanded ${{ is the subject
    case "$1" in
        *sh_checker_exclude*) return 0 ;;
        '>'* | '|'* | '['* | '{'* | '&'* | '*'*) return 0 ;;
        *\\*) return 0 ;;
        *'${{'*) return 0 ;;
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
    _again=$(clean_env sh "$1" 2>&1) && _again_status=0 || _again_status=$?
    _dirty=$(hostile_env sh "$1" 2>&1) && _dirty_status=0 || _dirty_status=$?

    # Two clean runs disagreeing about their own exit means the check
    # has no stable signal at all — noise, reported as sensitive so a
    # human looks, never as a clean bill built on a coin flip.
    [ "$_first_status" = "$_again_status" ] || return 0
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
    # An alias token directly before the colon can stand for any key
    # at all, so it joins the key alternation and the value side
    # decides; an expression value that textually carries the name
    # counts, and evaluating expressions is deliberately out of scope.
    grep -cE "(\"name\"|'name'|name|\"[^\"]*\\\\[^\"]*\"|\*[^:[:space:]]+)[[:space:]]*:[[:space:]]*(\"$name_re\"|'$name_re'|${name_re}[[:space:]]*([]#,}].*)?\$|[>|&*!]|\"[^\"]*\\\\[^\"]*\"|\"[^\"]*\$|'[^']*\$|(Arch( Shellcheck( \+)?)?)?[[:space:]]*\$|[\"']?\\\$[{][{].*${name_re})|^[[:space:]]*[?:]([[:space:]]|\$)" || true
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
    pattern=$(relevance_pattern "$BASH_WORKFLOW")
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

@test "an aliased name key is not silently missed" {
    # An alias can BE the key: with `&k name` anchored elsewhere,
    # `*k: Arch Shellcheck + Shfmt` resolves to the same name
    # property, and no physical line spells a recognized key. An alias
    # token followed by a colon can stand for any key at all, so it
    # counts whenever its value could be the required name — the same
    # combining logic the backslash-quoted key uses. Both alias-key
    # spacings count, since YAML accepts the colon adjacent or spaced.
    alias_key_count=$(printf '%s\n' '    *job_name_key: Arch Shellcheck + Shfmt' |
        count_arch_lint_names)
    alias_spaced_count=$(printf '%s\n' '    *job_name_key : Arch Shellcheck + Shfmt' |
        count_arch_lint_names)
    [ "$alias_key_count" -ge 1 ]
    [ "$alias_spaced_count" -ge 1 ]
}

@test "an expression carrying the name is not silently missed" {
    # GitHub evaluates expressions in names after YAML resolves, so an
    # expression value that textually carries the required name
    # reaches branch protection as exactly that name while the scalar
    # the count reads is neither the literal nor any refused spelling.
    # What the count deliberately does not attempt is evaluating
    # expressions: a name constructed without its text appearing —
    # format(), join(), an env lookup — is invisible to any line read,
    # and that boundary is stated here rather than implied away.
    # shellcheck disable=SC2016 # the unexpanded ${{ is the subject
    expr_count=$(printf 'name: "${{ %s }}"\n' "'Arch Shellcheck + Shfmt'" |
        count_arch_lint_names)
    [ "$expr_count" -ge 1 ]
}

@test "an aliased exclusion key is seen and refused" {
    # With `&k sh_checker_exclude` anchored elsewhere,
    # `*k: "ani-cli tests/arch"` resolves to the expected input while
    # no line spells the key. The declaration has to be counted and
    # selected, and then refused — an alias key could stand for
    # anything, and the list behind it cannot be read from this line.
    alias_excl="$BATS_TEST_TMPDIR/alias-key-excl.yml"
    printf '%s\n' '          *exclude_key: "ani-cli tests/arch"' >"$alias_excl"
    alias_excl_count=$(exclusion_declarations "$alias_excl")
    alias_excl_tokens=$(select_exclusion_line "$alias_excl" | parse_exclusions)
    [ "${alias_excl_count:-0}" -ge 1 ]
    exclusions_unreadable "$alias_excl_tokens"
}

@test "an expression-valued exclusion is refused, not scanned" {
    # The action evaluates expressions in its inputs: an exclusion
    # spelled as an expression reaches the linter as the resolved
    # list, while the extraction hands back the unresolved expression
    # and its whitespace-split tokens match nothing. An expression is
    # a value this read cannot resolve — refused, like every other
    # spelling past the extraction's boundary.
    # shellcheck disable=SC2016 # the unexpanded ${{ is the subject
    expr_excl=$(printf 'sh_checker_exclude: "${{ %s }}"\n' "'ani-cli tests/arch'" |
        parse_exclusions)
    exclusions_unreadable "$expr_excl"
}

@test "a name-holding job cannot borrow another job's lint step" {
    # The counts have to hold within the job that carries the name,
    # not across the file: an echo-only job holding the required name
    # beside a second job invoking the action with the sole exclude
    # list satisfies branch protection while the lint runs somewhere
    # branch protection never looks.
    split="$BATS_TEST_TMPDIR/split-lint.yml"
    printf '%s\n' 'on: push' 'jobs:' '  stub:' \
        '    name: Arch Shellcheck + Shfmt' '    runs-on: ubuntu-latest' \
        '    steps:' '      - run: echo done' \
        '  real-lint:' '    runs-on: ubuntu-latest' '    steps:' \
        '      - uses: luizm/action-sh-checker@master' \
        '        with:' \
        '          sh_checker_exclude: "ani-cli"' >"$split"
    run ! lint_step_present "$split"
}

@test "a commented action mention does not count as the lint step" {
    # A mention is not a step: a comment spelling the action satisfies
    # an unanchored match, and an env key can spell the input name
    # without the action ever reading it — an echo-only job dressed
    # as the lint.
    commented="$BATS_TEST_TMPDIR/commented-lint.yml"
    printf '%s\n' 'on: push' 'jobs:' '  arch-sh-checker:' \
        '    name: Arch Shellcheck + Shfmt' '    runs-on: ubuntu-latest' \
        '    env:' '      sh_checker_exclude: "ani-cli"' \
        '    steps:' \
        '      # uses: luizm/action-sh-checker@master' \
        '      - run: echo done' >"$commented"
    run ! lint_step_present "$commented"
}

@test "an escaped filter key cannot hide a path filter" {
    # A double-quoted key carrying an escape resolves to paths while
    # spelling none of its letters where a literal search looks. Any
    # key spelling the reading cannot resolve reads as a filter being
    # hidden, not as its absence.
    gated="$BATS_TEST_TMPDIR/escaped-paths-gate.yml"
    printf '%s\n' 'on:' '  pull_request:' '    "pa\x74hs":' \
        '      - "src/**"' 'permissions:' '  contents: read' >"$gated"
    run ! unconditional "$gated"
}

@test "the resolved workflow structure certifies what the text layer cannot" {
    # The text readings are one layer, each covering the spellings it
    # names, and YAML has more: `<<: *defaults` delivers the required
    # name into a job while no line of the job spells a name, and a
    # lookalike action repository satisfies a prefix match while being
    # somebody else's code. Resolution reads neither spelling — it
    # reads the resolved structure, where both collapse to the values
    # GitHub sees.
    certified_by_resolution "$REPO_ROOT/.github/workflows"
    merge_dir="$BATS_TEST_TMPDIR/merge-key-producer"
    mkdir "$merge_dir"
    cat >"$merge_dir/covert.yml" <<'YAML'
defs: &d
  name: Arch Shellcheck + Shfmt
"on": push
jobs:
  stub:
    <<: *d
    runs-on: ubuntu-latest
    steps:
      - run: echo done
YAML
    run ! certified_by_resolution "$merge_dir"
    look_dir="$BATS_TEST_TMPDIR/lookalike-action"
    mkdir "$look_dir"
    cat >"$look_dir/covert.yml" <<'YAML'
"on": push
jobs:
  stub:
    name: Arch Shellcheck + Shfmt
    runs-on: ubuntu-latest
    steps:
      - uses: luizm/action-sh-checker-evil@v1
        with:
          sh_checker_exclude: "ani-cli"
YAML
    run ! certified_by_resolution "$look_dir"
}

@test "a bracket-bearing checkout path enumerates literally" {
    # A checkout path is data, not pattern syntax: a directory
    # carrying a glob metacharacter — nothing stops anyone cloning
    # into ani-[gui] — must enumerate exactly like any other, or the
    # certification refuses an innocent checkout for a reason that
    # has nothing to do with its workflows. The same portability
    # property the apostrophe cases pin for the shell.
    bracket_dir="$BATS_TEST_TMPDIR/br[a]cket"
    mkdir "$bracket_dir"
    cat >"$bracket_dir/producer.yml" <<'YAML'
"on": pull_request
jobs:
  arch-sh-checker:
    name: Arch Shellcheck + Shfmt
    runs-on: ubuntu-latest
    steps:
      - uses: luizm/action-sh-checker@master
        with:
          sh_checker_exclude: "ani-cli"
YAML
    certified_by_resolution "$bracket_dir"
}

@test "a lint that cannot gate pull requests is refused, not certified" {
    # Counting the step is not enough: a step carrying if: false
    # never runs, one carrying continue-on-error: true cannot fail
    # the job, a workflow_dispatch-only trigger never reports on pull
    # requests, and a pull_request event restricted to chosen types
    # skips the pushes branch protection exists to gate. Each shape
    # is valid YAML and resolves cleanly.
    gate_dir="$BATS_TEST_TMPDIR/ungateable"
    mkdir "$gate_dir"
    cat >"$gate_dir/producer.yml" <<'YAML'
"on": pull_request
jobs:
  arch-sh-checker:
    name: Arch Shellcheck + Shfmt
    runs-on: ubuntu-latest
    steps:
      - uses: luizm/action-sh-checker@master
        if: false
        with:
          sh_checker_exclude: "ani-cli"
YAML
    run ! certified_by_resolution "$gate_dir"
    cat >"$gate_dir/producer.yml" <<'YAML'
"on": pull_request
jobs:
  arch-sh-checker:
    name: Arch Shellcheck + Shfmt
    runs-on: ubuntu-latest
    steps:
      - uses: luizm/action-sh-checker@master
        continue-on-error: true
        with:
          sh_checker_exclude: "ani-cli"
YAML
    run ! certified_by_resolution "$gate_dir"
    cat >"$gate_dir/producer.yml" <<'YAML'
"on": workflow_dispatch
jobs:
  arch-sh-checker:
    name: Arch Shellcheck + Shfmt
    runs-on: ubuntu-latest
    steps:
      - uses: luizm/action-sh-checker@master
        with:
          sh_checker_exclude: "ani-cli"
YAML
    run ! certified_by_resolution "$gate_dir"
    cat >"$gate_dir/producer.yml" <<'YAML'
"on":
  pull_request:
    types: [labeled]
jobs:
  arch-sh-checker:
    name: Arch Shellcheck + Shfmt
    runs-on: ubuntu-latest
    steps:
      - uses: luizm/action-sh-checker@master
        with:
          sh_checker_exclude: "ani-cli"
YAML
    run ! certified_by_resolution "$gate_dir"
}

@test "a yaml-less python3 shim on PATH does not defeat the certification" {
    # The interpreter on PATH is not always the one the package
    # manager provisioned: a pyenv shim or virtualenv python3 without
    # PyYAML shadows /usr/bin/python3, and a certification that
    # trusts the shim refuses on machines where the dependency is
    # installed. The shim runs the real interpreter with -S so
    # site-packages never load.
    shim_dir="$BATS_TEST_TMPDIR/python-shim"
    mkdir "$shim_dir"
    cat >"$shim_dir/python3" <<'SHIM'
#!/bin/sh
exec /usr/bin/python3 -S -E "$@"
SHIM
    chmod +x "$shim_dir/python3"
    PATH="$shim_dir:$PATH" certified_by_resolution "$REPO_ROOT/.github/workflows"
}

@test "a status-flapping check is flagged, not certified" {
    # A check whose output repeats while its exit status flaps between
    # identical runs passes the reproducibility gate, and the status
    # comparison that follows is built on a coin flip. Noise reads as
    # sensitive so a human looks.
    flapping="$BATS_TEST_TMPDIR/flapping-check.sh"
    cat >"$flapping" <<'FLAP'
#!/bin/sh
marker="$0.marker"
if [ -e "$marker" ]; then
    rm -f "$marker"
    exit 1
fi
: >"$marker"
exit 0
FLAP
    env_sensitive "$flapping"
}

@test "the bats job runs when the gitignore contract changes" {
    # deferral_record_entry_kind.bats asserts that .planning/ is
    # ignored, so .gitignore is an input of the ported suite: a PR
    # touching only it must count as relevant, or the case that reads
    # it goes green by never running.
    pattern=$(relevance_pattern "$BASH_WORKFLOW")
    [ -n "$pattern" ]
    run grep -qE "^($pattern)" <<<'.gitignore'
    [ "$status" -eq 0 ]
}

@test "a diagnostic grep cannot shadow the governing relevance pattern" {
    # The relevance probes have to read the pattern from the
    # conditional that actually sets relevant=true. A first-match
    # extraction reads whatever grep happens to come earlier: a
    # harmless diagnostic matching every path shadows a governing
    # predicate that quietly dropped tests/arch, and every relevance
    # probe stays green while the job no-ops.
    shadowed="$BATS_TEST_TMPDIR/shadowed-bash.yml"
    {
        sed '/if grep -qE/i\
          echo "$changed" | grep -qE '"'"'^(everything)'"'"' || true
' "$BASH_WORKFLOW"
    } >"$shadowed"
    pattern=$(relevance_pattern "$shadowed")
    [ -n "$pattern" ]
    run grep -qE "^($pattern)" <<<'tests/arch/deferral_record.sh'
    [ "$status" -eq 0 ]
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

@test "a quoted trigger key cannot hide a path filter" {
    # "on": declares the same trigger the bare key declares, but a
    # reading that recognizes only the bare spelling finds no block at
    # all — an empty range contains no paths, so a path-gated workflow
    # reads as unconditional, the failing-open direction. A trigger
    # block that cannot be located must read as conditional.
    gated="$BATS_TEST_TMPDIR/quoted-on-gate.yml"
    printf '%s\n' '"on":' '  pull_request:' '    paths:' \
        '      - "src/**"' 'permissions:' '  contents: read' >"$gated"
    run ! unconditional "$gated"
}

@test "the name-holding job invokes the lint action with its exclude list" {
    # Carrying the required name is not linting. A job that keeps the
    # name but drops the sh-checker step satisfies branch protection
    # while inspecting nothing, and with no exclusion declared the
    # coverage case reads empty tokens as "nothing excluded" — an
    # echo-only job certified as the lint. The workflow has to invoke
    # the action, exactly once, and declare its exclude list, exactly
    # once.
    stripped="$BATS_TEST_TMPDIR/stripped-lint.yml"
    printf '%s\n' 'on: push' 'jobs:' '  arch-sh-checker:' \
        '    name: Arch Shellcheck + Shfmt' '    runs-on: ubuntu-latest' \
        '    steps:' '      - run: echo done' >"$stripped"
    lint_step_present "$WORKFLOW"
    run ! lint_step_present "$stripped"
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
    pattern=$(relevance_pattern "$BASH_WORKFLOW")
    [ -n "$pattern" ]
    run grep -qE "^($pattern)" <<<'tests/arch/boundaries.sh'
    [ "$status" -eq 0 ]
}

@test "the bats job's relevance pattern does not match everything" {
    # A pattern that selects any path at all would satisfy the case
    # above while saying nothing about arch coverage.
    pattern=$(relevance_pattern "$BASH_WORKFLOW")
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
    pattern=$(relevance_pattern "$BASH_WORKFLOW")
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
    pattern=$(relevance_pattern "$BASH_WORKFLOW")
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
