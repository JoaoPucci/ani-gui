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

# Whether a workflow directory equals the blessed snapshot. The
# projection lives in one place — tests/tools/workflow-snapshot.py,
# which both this comparison and the regeneration run — so the two
# sides cannot drift apart into disagreeing about what is recorded.
# Regenerating is deliberately a human act with no seam here: no
# environment variable makes this rewrite its own expectations, which
# is the defect it exists to catch in other checks.
#
#   python3 tests/tools/workflow-snapshot.py .github/workflows \
#       > tests/arch/workflows.snapshot.json
workflows_match_snapshot() {
    _snap_py=$(resolution_python) || {
        printf 'snapshot gate unavailable: no python3 with PyYAML\n' >&2
        return 1
    }
    _generated=$("$_snap_py" "$REPO_ROOT/tests/tools/workflow-snapshot.py" "$1" 2>&1) || {
        printf '%s\n' "$_generated" >&2
        return 1
    }
    # An empty projection would compare equal to an empty snapshot and
    # certify nothing; neither side is allowed to be empty.
    [ -n "$_generated" ] || return 1
    [ -s "$REPO_ROOT/tests/arch/workflows.snapshot.json" ] || return 1
    printf '%s\n' "$_generated" |
        diff -u "$REPO_ROOT/tests/arch/workflows.snapshot.json" - >&2
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

@test "the workflows equal the blessed snapshot" {
    # Every reading in this file asks whether a workflow could be
    # spelled so as to defeat it, and that question has no end: YAML
    # and Actions both have more syntax than any finite reading
    # covers, so each answered spelling leaves the next one open. The
    # question that ends is whether the workflows are the ones a
    # human blessed — equality against a recorded snapshot, where any
    # change of any spelling is a difference and stops the run.
    workflows_match_snapshot "$REPO_ROOT/.github/workflows"
    tampered="$BATS_TEST_TMPDIR/tampered-workflows"
    mkdir "$tampered"
    cp "$REPO_ROOT/.github/workflows"/*.yml "$tampered/"
    cat >"$tampered/zz-second-producer.yml" <<'YAML'
"on": pull_request
jobs:
  stub:
    name: Arch Shellcheck + Shfmt
    runs-on: ubuntu-latest
    steps:
      - run: echo done
YAML
    run ! workflows_match_snapshot "$tampered"
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

@test "the bats job runs when the snapshot generator changes" {
    # workflows_match_snapshot executes tests/tools/workflow-snapshot.py,
    # which makes the generator a subject under test: a change touching
    # only it must select the bats job, or a broken projection lands
    # with the case that runs it skipped as irrelevant — and the
    # snapshot gate certifies workflows against a generator nobody ran.
    pattern=$(relevance_pattern "$BASH_WORKFLOW")
    [ -n "$pattern" ]
    run grep -qE "^($pattern)" <<<'tests/tools/workflow-snapshot.py'
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
@test "a check pointed at a missing file exits nonzero" {
    # A tool failing inside the pipeline must not leave the check
    # reporting success. `set -e` takes the failing command
    # substitution today; this holds it there.
    run sh "$ARCH_DIR/deferral_record.sh" /definitely/not/here
    [ "$status" -ne 0 ]
}
