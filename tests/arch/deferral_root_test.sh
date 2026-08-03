#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC2312 — command substitutions are read for their text, and a
#       failure arrives as an empty result that the assertion then catches
#   SC2310 — helpers are invoked in `if` conditions on purpose, so a
#       failing case reports rather than aborting the run
#
# Scoped to this file rather than widened in SHELLCHECK_OPTS, which
# would also relax the checks guarding the `ani-cli` script itself.
# shellcheck disable=SC2310,SC2312
# The checks must resolve this repository regardless of how they are
# invoked, and must not be redirectable by a stray environment.
#
# Both properties were broken in turn by the same seam. The library
# re-derived its own root from `$0`, which is meaningless once a caller
# has changed directory, so an invocation by relative path from
# `tests/arch` walked above the repository. Honouring an inherited
# `REPO_ROOT` fixed that and introduced the other half: `REPO_ROOT` is
# a common name, so any environment defining one redirected the check
# — silently, exiting 0 against the wrong tree.
#
# Only `ARCH_REPO_ROOT` overrides now, and callers hand the resolved
# root over under that name. These are the cases that pins it.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

failed=0

# This file's own scratch, removed on every exit path including an
# interrupt. It had none: the apostrophe clone used `mktemp -d` and an
# explicit `rm -rf` that only ran when the block completed.
#
# Everything this run creates is named from one prefix, and the prefix
# is a literal fixed before anything exists. That is the whole point,
# and it is not the same as installing the traps early. A name that
# comes back from a creating command cannot be registered in advance
# under any ordering: `mktemp -d` puts the directory on disk when it
# returns and the variable holds the path only once the substitution
# completes, so a signal in that interval leaves something behind that
# the trap has no way to name. Registering a prefix removes the
# dependency instead of racing it — the cleanup refers to nothing a
# creation produced, so there is no interval during which it is
# uninformed.
#
# `$$` keeps concurrent runs apart. The collision-freedom `mktemp` was
# there for is kept by allocating with `set -C`, which fails rather
# than opens when the path is already taken.
#
# `ARCH_DEFERRAL_TMP_PREFIX` exists so the cases below can hand this a
# prefix they control.
tmp_prefix="${ARCH_DEFERRAL_TMP_PREFIX:-$REPO_ROOT/tests/arch/.deferral-root.$$}"

cleanup() {
    rm -rf "$tmp_prefix" "$tmp_prefix".*
}

# Registering the prefix before creating anything buys the guarantee
# above and costs the opposite one: the cleanup sweeps the whole
# prefix, so anything already sitting there goes with it. A pid comes
# round again after a kill that skipped cleanup, so that is reachable
# rather than theoretical, and it has two shapes.
#
# The prefix itself occupied fails loudly — `mkdir` refuses and the
# run dies with the trap armed. A sibling merely sharing the name
# fails quietly, which is worse: the directory does not exist, `mkdir`
# succeeds, the run reports success and removes somebody else's file
# on the way out.
#
# Both are refused here, before anything is armed. Tracking only what
# this run created is the other way to answer it, and it is the way
# that was already tried — a manifest cannot be written until the path
# it names exists, which is the interval this whole arrangement is
# built to remove.
# `test -e` follows the link and answers about the target, so a
# dangling symlink reads as free — survives the check, fails the
# `mkdir`, and is then removed by the cleanup this refusal exists to
# prevent. `-L` asks about the link itself, which is the thing that is
# actually in the way.
prefix_taken=""
if [ -e "$tmp_prefix" ] || [ -L "$tmp_prefix" ]; then
    prefix_taken="$tmp_prefix"
else
    for existing in "$tmp_prefix".*; do
        if [ -e "$existing" ] || [ -L "$existing" ]; then
            prefix_taken="$existing"
            break
        fi
    done
fi
if [ -n "$prefix_taken" ]; then
    printf 'arch/deferral_root_test: %s already exists — refusing, because this run cleans by prefix and would remove a path it did not create\n' \
        "$prefix_taken" >&2
    exit 1
fi

trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

# The sabotage probe has to sit beside the original — the script
# derives its root from its own path, so a copy one level deeper
# resolves to `tests/` — which is where the prefix already is.
make_sabotage_probe() {
    # A counter in a variable would not survive. This is called inside
    # command substitutions, and a subshell's increment is lost, so two
    # calls in one line would hand back the same path. The candidate is
    # tried against the filesystem instead, and `set -C` makes the
    # create-if-absent atomic — no window in which two runs agree a
    # name is free.
    _n=0
    while [ "$_n" -lt 1000 ]; do
        _n=$((_n + 1))
        _probe="$tmp_prefix.probe.$_n"
        if (
            set -C
            : >"$_probe"
        ) 2>/dev/null; then
            printf '%s\n' "$_probe"
            return 0
        fi
    done
    return 1
}

# Named above, created here. Plain `mkdir` refuses a path that already
# exists, including a symlink planted at it.
scratch_dir="$tmp_prefix"
mkdir "$scratch_dir"

# Probe mode: the run re-executes itself with this set so the leak
# case watches a real process take a real signal, rather than reading
# trap definitions and hoping they mean what they say. It exits before
# the body, so nothing here re-enters.
if [ -n "${ARCH_SABOTAGE_LEAK_PROBE:-}" ]; then
    # Basenames only. An absolute path carries the checkout prefix,
    # and a newline anywhere in that prefix splits one record into
    # several — the reader then matches nothing. The basename is
    # `.deferral-root.<pid>.probe.<n>`, an alphabet this run controls.
    _p1=$(make_sabotage_probe)
    _p2=$(make_sabotage_probe)
    printf '%s\n' "${_p1##*/}" "${_p2##*/}"
    sleep 5
    exit 0
fi

# The nested clone re-runs this file; it must not recurse. The guard
# carries this file's own name because a generic one is readable from
# any environment: an exported `SKIP_NESTED` — plausible in a shell
# where some other suite uses it — silently removed the apostrophe
# case and left the run green, since the guarded branch prints
# nothing either way.
#
# This is the same defect as the `REPO_ROOT` override two checks up,
# in the same file, reached through a different door. A generic
# variable name is an input from the environment whether or not it was
# meant as one.

# No `eval`. Building a command string means quoting the repository
# path into it, and a checkout under a directory containing an
# apostrophe would then be re-parsed as shell syntax — a test that
# breaks on where it was cloned is the kind of thing this file exists
# to catch elsewhere.
expect_ok() {
    label=$1
    shift
    if "$@" >/dev/null 2>&1; then
        printf '  ok       %s\n' "$label"
    else
        printf '  FAIL     %s\n' "$label"
        failed=1
    fi
}

# Run a script from its own directory, by relative path.
from_arch() {
    (cd "$REPO_ROOT/tests/arch" && sh "./$1")
}

# Run a script with a hostile ambient root set.
with_stray_root() {
    REPO_ROOT=/tmp sh "$REPO_ROOT_ABS/tests/arch/$1"
}

REPO_ROOT_ABS=$REPO_ROOT

# The runner must not re-parse what it is given as shell syntax. A
# checkout under a directory containing an apostrophe is the case that
# breaks: building a command string interpolates the path into it, and
# the quote then closes a quote the runner opened. This asserts the
# property directly rather than waiting for the nested run to die of
# it further down.
quote_dir="$scratch_dir/o'brien-probe"
mkdir -p "$quote_dir"
printf '#!/bin/sh\nexit 0\n' >"$quote_dir/trivial.sh"
chmod +x "$quote_dir/trivial.sh"

printf 'arch/deferral_root_test: invocation and environment\n'

# Isolated in a subshell: the failure mode is a syntax error at
# evaluation time, which would otherwise kill this script outright and
# report nothing at all.
if (expect_ok probe sh "$quote_dir/trivial.sh") >/dev/null 2>&1; then
    printf '  ok       the runner handles a path containing an apostrophe\n'
else
    printf '  FAIL     the runner breaks on a path containing an apostrophe\n'
    failed=1
fi

for script in deferral_record agents_contract deferral_record_test; do
    expect_ok "$script.sh resolves the repository when invoked by relative path" \
        from_arch "$script.sh"
done

# Shell in this repository has to be linted by the shell linter, and
# the green has to be attributable. `Shellcheck + Shfmt` is reported
# by three workflows — the vendored `ani-cli checks`, its instant
# inverse stub, and the unconditional mirror whose exclude list
# covers all of `tests` — and branch protection is satisfied by the
# first success under a required name. On a pull request touching
# only the arch checks, a stub's echo can land before the real lint
# finishes, so a shellcheck failure in these load-bearing scripts
# could still merge. no-awk-required.yml documents the same race and
# resolves it the same way: a name nothing else answers for.
arch_lint_workflow="$REPO_ROOT/.github/workflows/arch-lint.yml"
arch_lint_name='Arch Shellcheck + Shfmt'

# The authority behind every text reading above: parse the workflows
# with a real parser and certify the RESOLVED structure, where
# quoting, escapes, folding, flow style, aliases and merge keys have
# already collapsed to the values GitHub sees. The text arms remain
# as the conservative belt — each fails closed on the spellings it
# names — but a spelling none of them names lands here, because
# resolution does not read spellings at all.
#
# Certified: exactly one job across the workflows resolves to the
# required name; its workflow's trigger carries no paths filter at
# any depth; the job invokes exactly one step whose action repository
# is exactly luizm/action-sh-checker; and that step's
# sh_checker_exclude input is a readable literal whose tokens do not
# cover tests/arch. Refused rather than certified: unparseable files,
# expression-valued names or exclusions, and a missing parser — a
# certification that cannot run must not read as one that passed.
#
# Stated boundaries: PyYAML resolves YAML 1.1, where a bare `on` key
# reads as boolean True — handled explicitly — and GitHub's parser
# differs at edges; a divergence surfaces as a refusal or a wrong
# producer count, never as a silent pass, because the certification
# demands exactly one well-formed producer.
# The interpreter that runs the certification: PATH's python3 when it
# can import yaml — a virtualenv that provides PyYAML is the
# environment's choice — otherwise /usr/bin/python3, where the Debian
# package installs it. A shim without the module no longer answers
# for the provisioned interpreter beside it; when neither can import
# yaml, the missing-parser refusal stands.
resolution_python() {
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
# environment variable makes this check rewrite its own expectations,
# which is the defect it exists to catch in other checks.
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
    "$_resolution_py" - "$1" "$arch_lint_name" <<'PYCERT'
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

# The text readings above are one layer: conservative, fail-closed,
# and by construction incomplete — YAML's spelling space is unbounded
# and each reading covers the spellings it names. The authority is
# resolution: parse every workflow with a real parser and certify the
# RESOLVED structure, where quoting, escapes, folding, flow, aliases
# and merge keys have already collapsed to the values GitHub sees. A
# merge key is the probe because no text arm can see it: the required
# name arrives in a job through `<<: *defaults` while no line of the
# job spells a name at all.
merge_dir="$scratch_dir/merge-key-producer"
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
merge_caught=0
if ! certified_by_resolution "$merge_dir" 2>/dev/null; then
    merge_caught=1
fi
live_certified=0
if certified_by_resolution "$REPO_ROOT/.github/workflows" 2>/dev/null; then
    live_certified=1
fi
if [ "$merge_caught" -eq 1 ] && [ "$live_certified" -eq 1 ]; then
    printf '  ok       the resolved workflow structure certifies what the text layer cannot\n'
else
    printf '  FAIL     resolution reads merge-key-caught=%s live-certified=%s — a resolved second producer is invisible\n' \
        "$merge_caught" "$live_certified"
    failed=1
fi
rm -rf "$merge_dir"

# The interpreter on PATH is not always the one the package manager
# provisioned: a pyenv shim or virtualenv python3 without PyYAML
# shadows /usr/bin/python3, and a certification that trusts the shim
# refuses on every such machine for want of a module that is
# installed. The certification has to find an interpreter that can
# import yaml before giving up.
shim_dir="$scratch_dir/python-shim"
mkdir "$shim_dir"
cat >"$shim_dir/python3" <<'SHIM'
#!/bin/sh
exec /usr/bin/python3 -S -E "$@"
SHIM
chmod +x "$shim_dir/python3"
shim_certified=0
if PATH="$shim_dir:$PATH" certified_by_resolution "$REPO_ROOT/.github/workflows" 2>/dev/null; then
    shim_certified=1
fi
if [ "$shim_certified" -eq 1 ]; then
    printf '  ok       a yaml-less python3 shim on PATH does not defeat the certification
'
else
    printf '  FAIL     a PATH shim without PyYAML fails the certification despite the provisioned interpreter
'
    failed=1
fi
rm -rf "$shim_dir"

# Everything above asks whether a workflow could be spelled so as to
# defeat a reading. That question has no end: YAML and Actions both
# have more syntax than any finite reading covers, and each answered
# spelling leaves the next one open. The question with an end is
# whether the workflows are the ones a human blessed — equality
# against a recorded snapshot, where any change of any spelling is a
# difference and stops the run until somebody looks.
#
# The snapshot records what matters and nothing else: every
# workflow's resolved job names, so a second job answering for the
# required check name is a difference whatever syntax produced it,
# and the arch lint workflow's whole resolved structure, so a
# skipped step, a tolerated failure, a narrowed trigger or a
# swapped action is a difference too. A step added to an unrelated
# workflow is not, which keeps the pin off everyday work.
snapshot_live=0
if workflows_match_snapshot "$REPO_ROOT/.github/workflows"; then
    snapshot_live=1
fi
tampered_dir="$scratch_dir/tampered-workflows"
mkdir "$tampered_dir"
cp "$REPO_ROOT/.github/workflows"/*.yml "$tampered_dir/"
cat >"$tampered_dir/zz-second-producer.yml" <<'YAML'
"on": pull_request
jobs:
  stub:
    name: Arch Shellcheck + Shfmt
    runs-on: ubuntu-latest
    steps:
      - run: echo done
YAML
tampered_caught=0
if ! workflows_match_snapshot "$tampered_dir" 2>/dev/null; then
    tampered_caught=1
fi
rm -rf "$tampered_dir"
if [ "$snapshot_live" -eq 1 ] && [ "$tampered_caught" -eq 1 ]; then
    printf '  ok       the workflows equal the blessed snapshot\n'
else
    printf '  FAIL     snapshot gate reads live=%s tampered-caught=%s — an unblessed workflow change passes\n' \
        "$snapshot_live" "$tampered_caught"
    failed=1
fi

# Counting the step is not enough: a step carrying `if: false` never
# runs, one carrying `continue-on-error: true` cannot fail the job,
# and a trigger without a bare `pull_request` event never reports on
# the pull requests branch protection exists to gate. Each shape is
# valid YAML, resolves cleanly, and certifies a job that inspects
# nothing where it matters — so each is refused: the producer job and
# its lint step must be unconditional and failure-gating, and the
# trigger must include pull_request without restrictions.
gate_dir="$scratch_dir/ungateable"
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
skipped_refused=1
if certified_by_resolution "$gate_dir" 2>/dev/null; then
    skipped_refused=0
fi
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
soft_refused=1
if certified_by_resolution "$gate_dir" 2>/dev/null; then
    soft_refused=0
fi
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
dispatch_refused=1
if certified_by_resolution "$gate_dir" 2>/dev/null; then
    dispatch_refused=0
fi
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
restricted_refused=1
if certified_by_resolution "$gate_dir" 2>/dev/null; then
    restricted_refused=0
fi
if [ "$skipped_refused" -eq 1 ] && [ "$soft_refused" -eq 1 ] &&
    [ "$dispatch_refused" -eq 1 ] && [ "$restricted_refused" -eq 1 ]; then
    printf '  ok       a lint that cannot gate pull requests is refused, not certified
'
else
    printf '  FAIL     ungateable shapes certify: skipped=%s soft-fail=%s dispatch-only=%s restricted-pr=%s (1 = refused)
' "$skipped_refused" "$soft_refused" "$dispatch_refused" "$restricted_refused"
    failed=1
fi
rm -rf "$gate_dir"

# A checkout path is data, not pattern syntax: a directory carrying a
# glob metacharacter — nothing stops anyone cloning into ani-[gui] —
# must enumerate exactly like any other, or the certification refuses
# an innocent checkout for a reason that has nothing to do with its
# workflows. The same portability property the apostrophe cases pin
# for the shell, pinned for the enumeration.
bracket_dir="$scratch_dir/br[a]cket"
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
bracket_certified=0
if certified_by_resolution "$bracket_dir" 2>/dev/null; then
    bracket_certified=1
fi
if [ "$bracket_certified" -eq 1 ]; then
    printf '  ok       a bracket-bearing checkout path enumerates literally\n'
else
    printf '  FAIL     a glob metacharacter in the checkout path empties the enumeration\n'
    failed=1
fi
rm -rf "$bracket_dir"

# The parser the certification stands on has to be provisioned where
# the suite runs, or the refusal that guards against a missing parser
# fires on every fresh environment for want of a package rather than
# a defect. The CI workflow that runs this suite must install it, and
# the requirement is pinned here the same way the relevance cases pin
# bash.yml.
if grep -q 'python3-yaml' "$REPO_ROOT/.github/workflows/arch.yml"; then
    printf '  ok       the arch workflow provisions the parser the suite stands on\n'
else
    printf '  FAIL     nothing installs PyYAML where the architectural suite runs\n'
    failed=1
fi

# Whether a check can be redirected by the environment it runs in.
#
# Two have been, for real: `REPO_ROOT` and `SKIP_NESTED` each arrived
# from an ordinary shell and silently changed what a check did while it
# still exited 0. That is the property worth holding.
#
# It is held by running the check rather than by reading it. The
# earlier form audited the source text for names that looked ambient,
# which meant deciding from `grep` and `awk` which `$NAME` is a read,
# which assignment owns it, and which text is code at all. That
# question needs a shell parser, and it was assembled one review round
# at a time: comments, then quoting, then heredocs, then heredoc
# delimiters, then several heredocs per command, then line
# continuations, then command-scoped assignment prefixes — each
# arriving only once the previous had shipped, and each a defect in the
# fix before it.
#
# Running the check asks the question directly. It cannot grow new
# rules, because it has none.
#
# What it gives up is naming the offending variable. What it gains is
# an answer about behaviour rather than about spelling, and a check
# whose own correctness is obvious. `HOME`, `PATH` and `TMPDIR` are
# left alone below because these scripts legitimately need them.
#
# The coverage is exactly the list below, and the report says so. A
# check gating on a name absent from it is invisible here — the
# hostile run inherits or omits that name exactly as the clean run
# does — and the contract requires an incomplete check to state its
# boundary rather than imply coverage it does not have. When a new
# generic name bites, the remedy is one line: add it.
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

# Sensitive when a hostile environment changes either what the check
# prints or how it exits.
#
# A clean run happens twice first, to ask whether the check is
# reproducible against itself at all. `node_tool_tests.sh` is not — it
# prints durations — and comparing its output to anything reports a
# difference every time. For those the exit status is the only stable
# signal, so that is what gets compared. Found by this case on its
# first run, which is the sort of thing the text audit could never
# have seen.
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

# The hunt has to find something it is known to find, or the day it
# stops working it reports ok about every check at once.
if env_sensitive "$REPO_ROOT/tests/fixtures/arch/redirectable-check.sh"; then
    printf '  ok       a check a stray environment redirects is detected\n'
else
    printf '  FAIL     a check that reads SKIP_NESTED was not detected as sensitive\n'
    failed=1
fi

# A check whose exit status flaps between identical runs has no
# stable signal at all: its output repeats, so the reproducibility
# gate passes, and its status is then compared as though it meant
# something. The second clean run's status is measured for exactly
# this — two clean runs disagreeing about their own exit means the
# check is noise, and noise reads as sensitive so a human looks,
# rather than as a clean bill built on a coin flip.
flapping_check="$scratch_dir/flapping-check.sh"
cat >"$flapping_check" <<'FLAP'
#!/bin/sh
marker="$0.marker"
if [ -e "$marker" ]; then
    rm -f "$marker"
    exit 1
fi
: >"$marker"
exit 0
FLAP
flap_flagged=0
if env_sensitive "$flapping_check"; then
    flap_flagged=1
fi
rm -f "$flapping_check" "$flapping_check.marker"
if [ "$flap_flagged" -eq 1 ]; then
    printf '  ok       a status-flapping check is flagged, not certified\n'
else
    printf '  FAIL     a check whose exit flaps between clean runs reads as insensitive\n'
    failed=1
fi

# The stray environment the hunt exists to catch can just as easily be
# the one this suite itself runs under. A caller that already exports
# a hostile name hands it to the clean runs too: all three runs are
# then redirected alike, the difference vanishes, and a sensitive
# check reads as clean. The clean baseline has to shed the hostile
# names, not merely differ from them.
if (
    SKIP_NESTED=1
    export SKIP_NESTED
    env_sensitive "$REPO_ROOT/tests/fixtures/arch/redirectable-check.sh"
); then
    printf '  ok       an exported hostile name cannot poison the clean baseline\n'
else
    printf '  FAIL     a caller exporting SKIP_NESTED=1 hides the sensitivity the calibration proves detectable\n'
    failed=1
fi

# Every real check, run twice. The self-tests are skipped: they
# re-execute themselves, and several deliberately vary on exactly the
# environment this hands them.
env_strays=''
for env_check in "$REPO_ROOT"/tests/arch/*.sh; do
    case "$(basename "$env_check")" in
        run-all.sh | *_test.sh) continue ;;
        *) ;;
    esac
    if env_sensitive "$env_check"; then
        env_strays="$env_strays $(basename "$env_check")"
    fi
done
if [ -z "$env_strays" ]; then
    printf '  ok       no check varies under the hostile names this run injects\n'
else
    printf '  FAIL     a stray environment redirects these checks:%s\n' "$env_strays"
    failed=1
fi

# A checkout path containing an apostrophe. This exists because the
# first version of this file built commands as strings and evaluated
# them, so the path was re-parsed as shell syntax and the suite broke
# on where it had been cloned. Verified by hand at the time; asserted
# here so it cannot regress.
# Inside the repository and inside the run's own scratch directory,
# so the cleanup trap removes it on every exit path — including an
# interrupt, which the explicit `rm -rf` at the end of this block
# would miss.
apostrophe_dir="$scratch_dir/o'brien"

# Two properties of the scratch this case creates, checked before it
# is used. It must live inside the repository — the suite has no
# business writing outside the tree it is checking — and it must be
# under the cleanup trap, so an interrupt does not leave a clone in
# the working tree for the next `git status` to report.
case "$apostrophe_dir" in
    "$REPO_ROOT"/*)
        printf '  ok       the apostrophe clone is inside the repository\n'
        ;;
    *)
        printf '  FAIL     the apostrophe clone is outside the repository: %s\n' "$apostrophe_dir"
        failed=1
        ;;
esac
case "$apostrophe_dir" in
    "$scratch_dir"/*)
        printf '  ok       the apostrophe clone is under the cleanup trap\n'
        ;;
    *)
        printf '  FAIL     the apostrophe clone is not under the cleanup trap\n'
        failed=1
        ;;
esac
mkdir -p "$apostrophe_dir"
# Guarded so the nested run does not clone again — but only this
# block. Guarding the whole script made the nested run assert nothing
# and report success for starting up, which the case above now counts
# rather than trusts.
if [ -n "${ARCH_DEFERRAL_NESTED:-}" ]; then
    :
elif git clone -q --depth=1 "$REPO_ROOT" "$apostrophe_dir/repo" 2>/dev/null; then
    # The clone carries committed state only, so an uncommitted change
    # to any of these would go unexercised — the working-tree copies
    # are what this run is meant to be testing.
    cp "$REPO_ROOT/tests/arch/deferral_root_test.sh" \
        "$REPO_ROOT/tests/arch/deferral_record.sh" \
        "$REPO_ROOT/tests/arch/deferral_record_test.sh" \
        "$REPO_ROOT/tests/arch/agents_contract.sh" \
        "$apostrophe_dir/repo/tests/arch/"
    # The workflow is a subject too, now that cases read its trigger
    # and exclude list. Without this the nested run judges the
    # committed copy while the parent judges the tree, and they
    # disagree exactly when the tree is what changed.
    mkdir -p "$apostrophe_dir/repo/.github/workflows"
    cp "$REPO_ROOT/.github/workflows/arch-lint.yml" \
        "$apostrophe_dir/repo/.github/workflows/"
    # The snapshot and the script that projects it are subjects for
    # the same reason: the nested run must judge the tree's pin, not
    # the committed one, or the two disagree exactly when the pin is
    # what changed.
    mkdir -p "$apostrophe_dir/repo/tests/tools"
    cp "$REPO_ROOT/tests/arch/workflows.snapshot.json" \
        "$apostrophe_dir/repo/tests/arch/"
    cp "$REPO_ROOT/tests/tools/workflow-snapshot.py" \
        "$apostrophe_dir/repo/tests/tools/"
    # Count the assertions the nested run makes, rather than trusting
    # its exit status. A guard that skips the whole script would exit
    # zero having checked nothing, and this case would report success
    # for a process that merely started.
    # Both the exit status and the count. The status alone could pass a
    # run that skipped everything; the count alone could pass a run
    # where one case failed while five others succeeded.
    # `|| nested_status=$?` matters: a bare command substitution that
    # fails aborts the whole script under `set -e`, so a failing nested
    # run used to kill the parent silently instead of being reported.
    # The nested run also skips the sabotage case — it exists to prove
    # the apostrophe path works, not to re-run a sub-suite.
    nested_status=0
    nested_out=$(cd "$apostrophe_dir/repo" &&
        ARCH_DEFERRAL_NESTED=1 ARCH_DEFERRAL_NO_SABOTAGE=1 \
            sh tests/arch/deferral_root_test.sh 2>&1) || nested_status=$?
    nested_ok=$(printf '%s\n' "$nested_out" | grep -c '^  ok' || true)
    if [ "$nested_status" -eq 0 ] && [ "${nested_ok:-0}" -ge 5 ]; then
        printf '  ok       the suite asserts (%s cases) from a path containing an apostrophe\n' "$nested_ok"
    else
        printf '  FAIL     nested run: status %s, %s assertions (want 0 and >=5)\n' "$nested_status" "${nested_ok:-0}"
        failed=1
    fi
else
    # Not a skip. This clones a local path to a local path inside the
    # repository, with no network and no remote, so there is no benign
    # reason for it to fail — a failure means the environment cannot
    # do something this suite depends on, and reporting ok for that
    # turns a broken checkout into a green run.
    printf '  FAIL     could not clone for the apostrophe case\n'
    failed=1
fi

# A stray REPO_ROOT must not redirect anything. It used to, and the
# script exited 0 against the wrong tree rather than failing.
expect_ok "a stray REPO_ROOT does not redirect the record check" \
    with_stray_root deferral_record.sh
expect_ok "a stray REPO_ROOT does not redirect the contract check" \
    with_stray_root agents_contract.sh

# The library must honour a root the caller resolved, including when
# it is sourced rather than executed. Sourcing is the case that got
# missed: executed, the file derives its own root and that happens to
# be right; sourced by a test that has already pointed at a scratch
# repository, re-deriving silently `cd`s back to this checkout and the
# caller's choice is discarded without a word.
probe_repo=$(mktemp -d "$scratch_dir/sourced-root.XXXXXX")
(
    cd "$probe_repo" && git init -q . &&
        git config user.email t@e && git config user.name t
) >/dev/null 2>&1
sourced_root=$(
    ARCH_DEFERRAL_RECORD_LIB=1 ARCH_REPO_ROOT="$probe_repo" \
        sh -c '. "$1/tests/arch/deferral_record.sh"; pwd' _ "$REPO_ROOT" 2>/dev/null
) || sourced_root=''
case "$sourced_root" in
    "$probe_repo"*)
        printf '  ok       the sourced library stays in the caller resolved root\n'
        ;;
    *)
        printf '  FAIL     sourcing the library left the root at %s\n' \
            "${sourced_root:-unknown}"
        failed=1
        ;;
esac

# An environment that cannot build the clone must fail the run rather
# than report a skip. Nothing about this clone is allowed to fail
# benignly — it is a local path to a local path inside the repository,
# no network and no remote — so a failure means the environment cannot
# do something this suite depends on, and calling that ok states
# something untrue.
#
# Proved by sabotage: a copy of this file with an unsatisfiable
# `--reference` runs the clone for real and cannot complete it. The
# copy skips this case, or it would sabotage a copy of itself forever.
# A cancelled run must not leave probes behind. Every path the
# allocator hands out has to reach the trap, or a signal between
# allocation and the explicit removal litters the working tree with
# files the next `git status` reports.
#
# Two ways this case could pass without measuring anything, both
# guarded below. If the child dies before printing, there is nothing
# to look for and an unguarded loop reports success having checked no
# paths. And the paths are read a line at a time: a checkout under a
# directory with a space in it splits an unquoted expansion into
# fragments, so the test would stat names that were never files.
leak_out=$(mktemp "$scratch_dir/leak-probe.XXXXXX")
ARCH_SABOTAGE_LEAK_PROBE=1 sh "$REPO_ROOT/tests/arch/deferral_root_test.sh" \
    >"$leak_out" 2>&1 &
leak_pid=$!
i=0
while [ "$i" -lt 50 ] && [ "$(wc -l <"$leak_out")" -lt 2 ]; do
    sleep 0.1
    i=$((i + 1))
done
# The protocol first: every record the probe emits has to be spelled
# from an alphabet this run controls. An absolute path carries the
# checkout prefix, and a newline anywhere in that prefix splits one
# record into several — the reader then matches nothing and reports a
# failure about serialization, not about cleanup.
leak_protocol_ok=1
while IFS= read -r probe_line; do
    [ -n "$probe_line" ] || continue
    case "$probe_line" in
        */*) leak_protocol_ok=0 ;;
        *) ;;
    esac
done <"$leak_out"
if [ "$leak_protocol_ok" -eq 1 ]; then
    printf '  ok       the leak probe reports names, not paths\n'
else
    printf '  FAIL     the leak probe serializes absolute paths — a newline in the checkout path splits its records\n'
    failed=1
fi

# Verify the allocations exist while the child still holds them.
# Counting plausible-looking lines is not evidence: two fabricated
# paths, or the same one twice, satisfy any count. A path that is on
# disk now was really allocated, and checking before the signal is the
# only moment that is true.
leak_present=0
leak_seen=""
while IFS= read -r probe; do
    case "$probe" in
        .deferral-root.*.probe.*) ;;
        *) continue ;;
    esac
    case "$leak_seen" in
        *"[$probe]"*) continue ;;
        *) leak_seen="${leak_seen}[$probe]" ;;
    esac
    [ -e "$REPO_ROOT/tests/arch/$probe" ] && leak_present=$((leak_present + 1))
done <"$leak_out"

kill -TERM "$leak_pid" 2>/dev/null || true
leak_status=0
wait "$leak_pid" 2>/dev/null || leak_status=$?

# Count allocations, not lines. stderr is merged into this file, so
# two diagnostics satisfy a line count while nothing was ever
# allocated — the loop then finds neither string on disk and the case
# reports cleanup succeeded. A line only counts if it is a path the
# allocator could have produced.
leak_count=0
leak_found=0
while IFS= read -r probe; do
    case "$probe" in
        .deferral-root.*.probe.*) ;;
        *) continue ;;
    esac
    leak_count=$((leak_count + 1))
    if [ -e "$REPO_ROOT/tests/arch/$probe" ]; then
        leak_found=1
        rm -f "$REPO_ROOT/tests/arch/$probe"
    fi
done <"$leak_out"

if [ "$leak_present" -ne 2 ]; then
    printf '  FAIL     %s distinct probes existed before the signal, not 2 — nothing was allocated to clean up\n' \
        "$leak_present"
    failed=1
elif [ "$leak_status" -ne 143 ]; then
    printf '  FAIL     the leak probe exited %s, not 143 — it was not the signal that ended it\n' \
        "$leak_status"
    failed=1
elif [ "$leak_found" -eq 0 ]; then
    printf '  ok       a cancelled run leaves no probes behind\n'
else
    printf '  FAIL     a cancelled run left an allocated probe in the working tree\n'
    failed=1
fi

# The sabotage probe must not collide with a file a developer already
# has. A fixed name means `>` truncates whatever sits there and the
# cleanup trap deletes it; had it been a symlink, the redirection
# would have written through to its target. That is the hazard
# `make_untracked_probe` exists for, on the other probe, so the same
# answer applies: allocate a path, never assume one.
#
# Asserted against the allocator directly rather than by running the
# sabotage step, because that step re-executes this file — a case that
# re-enters it has to be reasoned about rather than merely written.
#
# `set -e` would take a missing allocator as a failure of the whole
# script, which is the condition being measured.
alloc_status=0
first_sabotage=$(make_sabotage_probe 2>/dev/null) || alloc_status=$?

if [ "$alloc_status" -ne 0 ] || [ -z "$first_sabotage" ]; then
    printf '  FAIL     no collision-free allocator for the sabotage probe\n'
    failed=1
else
    printf 'do not lose me\n' >"$first_sabotage"
    second_sabotage=$(make_sabotage_probe)
    if [ "$second_sabotage" = "$first_sabotage" ]; then
        printf '  FAIL     the sabotage probe returned the same path twice\n'
        failed=1
    elif [ "$(cat "$first_sabotage" 2>/dev/null)" != 'do not lose me' ]; then
        printf '  FAIL     the sabotage probe clobbered an existing file\n'
        failed=1
    else
        printf '  ok       the sabotage probe never reuses or truncates a path\n'
    fi

    # It also has to sit beside the original: the script derives its
    # root from its own path, so a copy one level deeper resolves to
    # `tests/` and fails for a reason unrelated to the measurement.
    case "$second_sabotage" in
        "$REPO_ROOT/tests/arch/"*)
            printf '  ok       the sabotage probe is allocated beside the original\n'
            ;;
        *)
            printf '  FAIL     the sabotage probe is not beside the original\n'
            failed=1
            ;;
    esac

    # Everything this run creates has to lie under one prefix, and that
    # prefix has to be a literal rather than something a creating
    # command handed back.
    #
    # This is what closes the window a careful ordering of statements
    # cannot. `dir=$(mktemp -d ...)` puts the directory on disk the
    # moment mktemp returns and fills `dir` only when the substitution
    # completes; a signal in between leaves a directory the trap has no
    # way to name, however early the trap was installed. A registered
    # prefix removes the dependency altogether — the cleanup refers to
    # nothing that a creation produced, so there is no interval during
    # which it is uninformed.
    #
    # Asserted over the paths themselves, since the ordering it stands
    # for is not observable after the fact.
    prefix_stem="$REPO_ROOT/tests/arch/.deferral-root.$$"
    prefix_stray=""
    for allocated in "$scratch_dir" "$first_sabotage" "$second_sabotage"; do
        case "$allocated" in
            "$prefix_stem" | "$prefix_stem".*) ;;
            *) prefix_stray="$prefix_stray $allocated" ;;
        esac
    done
    if [ -z "$prefix_stray" ]; then
        printf '  ok       every temporary lies under the one registered prefix\n'
    else
        printf '  FAIL     allocated outside the registered prefix:%s\n' \
            "$prefix_stray"
        failed=1
    fi

    # The other half of registering before creating. A prefix that is
    # already occupied belongs to somebody — a pid comes round again
    # after a kill that skipped cleanup — and the trap, armed before
    # anything was made, would hand it to `rm -rf` as soon as `mkdir`
    # failed. Refusing outright is what keeps the first guarantee from
    # buying the opposite one.
    # Skipped in the child, which carries the variable. A run that
    # refuses exits before reaching here, so the guard matters only
    # while the refusal is absent — which is exactly when this case is
    # meant to fail, and without it that failure is an unbounded fork
    # rather than a report.
    if [ -z "${ARCH_DEFERRAL_TMP_PREFIX:-}" ]; then
        occupied="$scratch_dir/occupied-prefix"
        mkdir -p "$occupied"
        printf 'not yours\n' >"$occupied/keep-me"
        occupied_status=0
        ARCH_DEFERRAL_TMP_PREFIX="$occupied" \
            sh "$REPO_ROOT/tests/arch/deferral_root_test.sh" >/dev/null 2>&1 ||
            occupied_status=$?
        if [ "$occupied_status" -ne 0 ] && [ -f "$occupied/keep-me" ]; then
            printf '  ok       an occupied prefix is refused, not adopted\n'
        else
            printf '  FAIL     an occupied prefix was adopted or emptied (status %s, keep-me %s)\n' \
                "$occupied_status" \
                "$([ -f "$occupied/keep-me" ] && echo present || echo gone)"
            failed=1
        fi

        # A file that merely shares the prefix is the harder half, and
        # it fails quietly. The prefix itself does not exist, so
        # `mkdir` succeeds and the run reports success — then removes
        # somebody else's file on the way out, because the cleanup
        # covers the whole prefix and not only what this run made.
        # Nothing in the output says so.
        shared="$scratch_dir/shared-prefix"
        printf 'not yours\n' >"$shared.probe.999"
        shared_status=0
        ARCH_DEFERRAL_TMP_PREFIX="$shared" \
            sh "$REPO_ROOT/tests/arch/deferral_root_test.sh" >/dev/null 2>&1 ||
            shared_status=$?
        if [ "$shared_status" -ne 0 ] && [ -f "$shared.probe.999" ]; then
            printf '  ok       a file sharing the prefix is refused, not swept\n'
        else
            printf '  FAIL     a file sharing the prefix was swept (status %s, sentinel %s)\n' \
                "$shared_status" \
                "$([ -f "$shared.probe.999" ] && echo present || echo gone)"
            failed=1
        fi

        # `test -e` follows the link and answers about the target, so a
        # dangling symlink reads as a free path. It survives the
        # refusal, fails the `mkdir`, and is then removed by the
        # cleanup the refusal exists to prevent — the one shape of
        # collision the check cannot see.
        dangling="$scratch_dir/dangling-prefix"
        ln -s /definitely/not/here "$dangling"
        dangling_status=0
        ARCH_DEFERRAL_TMP_PREFIX="$dangling" \
            sh "$REPO_ROOT/tests/arch/deferral_root_test.sh" >/dev/null 2>&1 ||
            dangling_status=$?
        if [ "$dangling_status" -ne 0 ] && [ -L "$dangling" ]; then
            printf '  ok       a dangling symlink at the prefix is refused, not removed\n'
        else
            printf '  FAIL     a dangling symlink at the prefix was adopted or removed (status %s, link %s)\n' \
                "$dangling_status" \
                "$([ -L "$dangling" ] && echo present || echo gone)"
            failed=1
        fi
    fi

    rm -f "$first_sabotage" "$second_sabotage"
fi

if [ -z "${ARCH_DEFERRAL_NO_SABOTAGE:-}" ]; then
    # Must sit beside the original, not inside the scratch directory:
    # the script derives the repository root from its own path, so a
    # copy one level deeper resolves to `tests/` and fails for a
    # reason that has nothing to do with the clone.
    sabotage_probe=$(make_sabotage_probe)
    sabotaged="$sabotage_probe"
    sed 's|git clone -q --depth=1|git clone -q --depth=1 --reference /nonexistent-ref|' \
        "$REPO_ROOT/tests/arch/deferral_root_test.sh" >"$sabotaged"
    # Same `set -e` trap as the nested run: this substitution is
    # expected to fail, and a bare one would abort this script instead
    # of letting the case report.
    sabotage_status=0
    sabotage_out=$(ARCH_DEFERRAL_NO_SABOTAGE=1 sh "$sabotaged" 2>&1) ||
        sabotage_status=$?
    if [ "$sabotage_status" -ne 0 ] &&
        printf '%s' "$sabotage_out" | grep -q 'could not clone'; then
        printf '  ok       a clone that cannot be built fails the run\n'
    else
        printf '  FAIL     an unbuildable clone left the run passing (status %s)\n' \
            "$sabotage_status"
        failed=1
    fi
fi

[ "$failed" -eq 0 ] || {
    printf 'arch/deferral_root_test: FAILED\n'
    exit 1
}
printf 'arch/deferral_root_test: ok\n'
