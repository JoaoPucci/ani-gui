#!/bin/sh

# Architectural invariant: only master saves an Actions cache, and
# every apt install goes through the cached install action.
#
# The repository's Actions caches share one 10 GB limit, and GitHub
# evicts the least recently used entry once it is reached. A cache a
# pull request saves is readable by that pull request alone, so every
# such copy is space taken from the caches master saves for everyone:
# when this check was written the limit was full, about half of it
# copies saved by pull-request branches, several of them already
# merged, and master's own caches were the ones being evicted. A pull
# request still restores master's caches; it only never writes one.
#
# Read from the workflows under .github/workflows/ and the composite
# actions under .github/actions/, through PyYAML, a real parser, so a
# step is a step whatever its spelling. Each of these is a finding:
#
#   - a Swatinem/rust-cache step whose `save-if` is not exactly
#     `${{ github.ref == 'refs/heads/master' }}`: the action saves
#     from every ref unless told otherwise;
#   - an actions/cache step (the combined form): its post step saves
#     from every ref, and it takes no condition. Restore with
#     actions/cache/restore and save with actions/cache/save;
#   - an actions/cache/save step whose `if` is not master-gated: the
#     expression has to begin `github.ref == 'refs/heads/master'`,
#     joined to anything further with `&&` alone, since one `||` lets
#     another ref through;
#   - an actions/setup-* step given a `cache` input: setup-node's
#     built-in cache saves from every ref, and has no setting that
#     stops it. .github/actions/pnpm-install is the master-only form;
#   - a call of that action without `working-directory: .`: every
#     caller installs from the workspace root, its arguments choosing
#     what to install, so no two callers that share a cache key differ
#     in where they run (composite steps ignore a job's defaults, so
#     the input is the only place the directory can be said);
#   - a `run` that installs with apt or apt-get anywhere but the
#     install action .github/actions/apt-install, whose download cache
#     the runner's slow mirror makes worth having in every job; and an
#     install action whose last installing step comes before its
#     restore (the plan step before the restore lists, with
#     --print-uris, what the install would fetch).
#
# What this does NOT check: an action other than these that caches on
# its own (a third-party action keeping a cache internally), or a
# script a step runs that calls apt or the cache service itself — only
# the steps' own text is read — or an apt install split across a line
# continuation so that no one line carries both `apt` and `install`,
# or one whose command name is quoted or held in a variable
# (`"apt-get" install`, `$APT install`):
# the install rule reads a run one line at a time, and joining shell
# lines is parsing shell, which AGENTS.md §2 puts out of bounds. A
# comment line spelling an install is a finding, which errs the safe
# way. Whether the install action's cache
# actually works is a runtime question; its report step prints how
# many planned files apt kept, in every job that uses it.

set -eu

REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$REPO_ROOT"

if [ ! -d .github/workflows ]; then
    printf 'arch/ci_caches: no .github/workflows directory — skipping\n'
    exit 0
fi

# PATH's python3 when it can import yaml, otherwise the distro's, where
# the python3-yaml package installs it. Neither is a failure, not a
# skip: a check that cannot parse has checked nothing.
py=''
for candidate in python3 /usr/bin/python3; do
    if "$candidate" -I -c 'import yaml' 2>/dev/null; then
        py=$candidate
        break
    fi
done
if [ -z "$py" ]; then
    printf 'arch/ci_caches FAIL: no python3 with PyYAML to parse the workflows (install python3-yaml)\n' >&2
    exit 1
fi

"$py" -I - <<'PY'
import glob
import posixpath
import re
import sys

import yaml

MASTER_SAVE_IF = "${{ github.ref == 'refs/heads/master' }}"
MASTER_REF = "github.ref == 'refs/heads/master'"
APT_ACTION = ".github/actions/apt-install/action.yml"
PNPM_DIR = "."
APT_INSTALL = re.compile(r"(^|[^\w-])apt(-get)?\s(.*\s)?install([^\w-]|$)")

findings = []


def steps_of(doc):
    """(where, step) for every step of a workflow's jobs or of a
    composite action, `where` naming the job and the step."""
    if not isinstance(doc, dict):
        return
    owners = []
    runs = doc.get("runs")
    if isinstance(runs, dict):
        owners.append(("", runs.get("steps")))
    jobs = doc.get("jobs")
    if isinstance(jobs, dict):
        owners.extend(("job %s, " % jid, job.get("steps")) for jid, job in jobs.items() if isinstance(job, dict))
    for owner, steps in owners:
        if not isinstance(steps, list):
            continue
        for index, step in enumerate(steps):
            if isinstance(step, dict):
                name = step.get("name") or step.get("uses") or "#%d" % (index + 1)
                yield "%sstep %s" % (owner, name), step


def master_gated(expr):
    if not isinstance(expr, str):
        return False
    body = expr.strip()
    if body.startswith("${{") and body.endswith("}}"):
        body = body[3:-2].strip()
    body = " ".join(body.split())
    if "||" in body:
        return False
    return body == MASTER_REF or body.startswith(MASTER_REF + " && ")


def check(path, doc):
    restore_at = None
    installs_at = []
    for index, (name, step) in enumerate(steps_of(doc)):
        where = "%s: %s" % (path, name)
        uses = step.get("uses")
        uses = uses if isinstance(uses, str) else ""
        # GitHub resolves an action's owner and name case-insensitively.
        action = uses.split("@", 1)[0].lower()
        inputs = step.get("with") if isinstance(step.get("with"), dict) else {}
        if action == "swatinem/rust-cache":
            if inputs.get("save-if") != MASTER_SAVE_IF:
                findings.append("%s saves its cache from every ref; give it\n    save-if: %s" % (where, MASTER_SAVE_IF))
        elif action == "actions/cache":
            findings.append("%s saves from every ref in its post step; split it into actions/cache/restore and a master-gated actions/cache/save" % where)
        elif action == "actions/cache/save":
            if not master_gated(step.get("if")):
                findings.append("%s is not master-gated; its if: has to begin %s, joined by && alone" % (where, MASTER_REF))
        elif action == "actions/cache/restore":
            if restore_at is None:
                restore_at = index
        elif action.startswith("actions/setup-") and "cache" in inputs:
            findings.append("%s caches through the setup action, which saves from every ref; drop `cache:` (pnpm: use ./.github/actions/pnpm-install)" % where)
        elif action.startswith("./") and posixpath.normpath(action) == ".github/actions/pnpm-install":
            if inputs.get("working-directory") != PNPM_DIR:
                findings.append("%s has to run its install from the workspace root; give it\n    working-directory: %s" % (where, PNPM_DIR))
        run = step.get("run")
        if isinstance(run, str) and any(APT_INSTALL.search(line) for line in run.splitlines()):
            if path != APT_ACTION:
                findings.append("%s installs with apt directly; use ./.github/actions/apt-install, which caches the download" % where)
            installs_at.append(index)
    if path == APT_ACTION and installs_at and (restore_at is None or restore_at > installs_at[-1]):
        findings.append("%s installs before any actions/cache/restore step" % path)


paths = sorted(
    glob.glob(".github/workflows/*.yml")
    + glob.glob(".github/workflows/*.yaml")
    + glob.glob(".github/actions/*/action.yml")
    + glob.glob(".github/actions/*/action.yaml")
)
for path in paths:
    with open(path, encoding="utf-8") as handle:
        for doc in yaml.safe_load_all(handle):
            check(path, doc)

for finding in findings:
    print("arch/ci_caches FAIL: " + finding, file=sys.stderr)
if findings:
    sys.exit(1)
print("arch/ci_caches PASS (%d files)" % len(paths))
PY
