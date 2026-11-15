#!/bin/sh
# Architectural invariant: no release's backend trusts the test-run key.
#
# A backend built with its `test-run-key` cargo feature trusts a test
# run's flags file (ANI_GUI_TEST_FLAGS_FILE) under a fixture key whose
# secret anyone can derive from
# backend/tests/fixtures/flags-signature/make.py. The tests enable it
# (`cargo test --all-features`), and so does the end-to-end suite's own
# package (electron's `package:e2e`); a release must never carry it. That a
# build with the feature still fetches the live file under its own keys
# alone is the backend's tests' to hold (flags/launch_keys_test.rs).
# What this check holds is narrower than "no release can carry it":
#
#   - the feature is not among the backend's default features, directly
#     or through another feature `default` turns on, read from
#     backend/Cargo.toml with Python's TOML parser;
#   - no script in electron/package.json or the root package.json that
#     runs `cargo build` passes --features, -F or --all-features, read
#     with Python's JSON parser and split into words by its shell
#     lexer, except electron's `build:backend:e2e`, which builds the
#     end-to-end suite's backend. That one must exist, must enable
#     exactly `test-run-key`, and must build into `target-e2e` with
#     `--target-dir`, a directory no release configuration reads
#     (package.json's `build` never names it; the e2e package's own
#     configuration is electron/lib/e2e-package.cjs, whose tests hold
#     where it reads from and writes to);
#   - only electron's `package:e2e` names `build:backend:e2e` or the
#     e2e package's configuration (electron-builder.e2e.cjs), only `e2e`
#     names `package:e2e`, only `build:backend:e2e` names `target-e2e`,
#     no script names `dist-e2e`, and no script runs `e2e` (the word
#     after pnpm, npm, yarn or `run`); a root script names none of them.
#     The names are matched as text anywhere in a script, quoted strings
#     included, so a script that only mentions one (a clean script
#     removing dist-e2e, say) is refused too and belongs in the allowed
#     list above if it is wanted. So no release script reaches the feature build, or
#     packages under that configuration with a target of its own (a CLI
#     target overrides the configuration's `dir`), or points a release
#     build at the e2e directories;
#   - no workflow line that runs `cargo build` passes a feature flag.
#
# It does not see other routes to a feature build: RUSTFLAGS or a
# .cargo/config.toml setting `--cfg feature="test-run-key"`, other
# spellings of the build (`cargo b`, `cargo +toolchain build`,
# `cargo rustc`, `cargo install`, a command continued across lines), a
# script that builds through a helper file it calls, a name spelled
# another way (built from parts, an environment variable, a glob), a
# script that runs `e2e` other than as the word after a package
# manager or `run`, anything outside package.json scripts (a workflow
# step, or a person, running package:e2e or build:backend:e2e, with or
# without a target appended, which makes an installable package that
# carries the test-run backend), or a script that packages a binary
# some other command left behind
# (`dist:win` packages whatever target/release holds; `package:win`
# builds it first).
#
# Both platforms' packaging scripts are covered, Windows's
# `build:backend` among them. CI also builds each platform's release
# backend with the release's own script and checks that the binary does
# not hold the key: on Linux the e2e workflow's release-shaped job
# ("The release-shaped backend trusts no test-run key"), with the e2e
# package's binary checked to hold it, so the search is seen to find
# it; on Windows a job of its own on windows-latest. This check holds
# that both jobs exist, build with the release's script and name the
# right binary (last rule below), not what their steps go on to do.
# Nothing builds the Windows installer in CI.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

failed=0

if ! python3 -I - backend/Cargo.toml <<'PY'; then
import sys, tomllib
with open(sys.argv[1], 'rb') as f:
    manifest = tomllib.load(f)
features = manifest.get('features', {})
if 'test-run-key' not in features:
    sys.exit('backend/Cargo.toml declares no test-run-key feature')
on, todo = set(), list(features.get('default', []))
while todo:
    name = todo.pop()
    if name in on or name not in features:
        continue
    on.add(name)
    todo.extend(features[name])
if 'test-run-key' in on:
    sys.exit('backend/Cargo.toml enables test-run-key by default')
PY
    printf 'arch/test_run_key FAIL: backend/Cargo.toml (see above)\n' >&2
    failed=1
fi

if ! python3 -I - package.json electron/package.json <<'PY'; then
import json, shlex, sys
FEATURE_FLAGS = ('--all-features', '--features', '-F')
E2E_BUILD = 'build:backend:e2e'
E2E_CONFIG = 'electron-builder.e2e.cjs'
# Which electron script may name each e2e step or e2e path; a root
# script may name none. Matched as text anywhere in the script, quoted
# strings included.
CALLERS = {
    E2E_BUILD: {'package:e2e'},
    'package:e2e': {'e2e'},
    E2E_CONFIG: {'package:e2e'},
    'target-e2e': {E2E_BUILD},
    'dist-e2e': set(),
}
# `e2e` itself is run by people, never by another script: refused as
# the word after a package manager or its `run`.
RUNNERS = ('pnpm', 'npm', 'yarn', 'run')


def words_of(cmd):
    # Operators (`&&`, `;`, `|`) come out as words of their own, so a
    # step glued to one is still seen.
    lex = shlex.shlex(cmd, posix=True, punctuation_chars=True)
    lex.whitespace_split = True
    return list(lex)


def runs_e2e(words):
    return any(w == 'e2e' and i > 0 and words[i - 1] in RUNNERS for i, w in enumerate(words))


bad = []
for path in sys.argv[1:]:
    electron = path.endswith('electron/package.json')
    with open(path, encoding='utf-8') as f:
        manifest = json.load(f)
    scripts = manifest.get('scripts', {})
    if 'target-e2e' in json.dumps(manifest.get('build', {})):
        bad.append(f'{path} "build": names target-e2e, the e2e backend\'s directory')
    for name, cmd in scripts.items():
        words = words_of(cmd)
        for step, callers in CALLERS.items():
            # `package:e2e` names build:backend:e2e's directory only
            # through that script, never as text of its own.
            if step in cmd and not (electron and name in callers):
                bad.append(f'{path} "{name}": names {step}')
        if runs_e2e(words):
            bad.append(f'{path} "{name}": runs e2e')
        if 'cargo build' not in cmd:
            continue
        flags = [w for w in words if w in FEATURE_FLAGS or w.startswith(('--features=', '-F'))]
        if not flags:
            continue
        if electron and name == E2E_BUILD:
            at = lambda w: words[words.index(w) + 1] if w in words and words.index(w) + 1 < len(words) else None
            if flags != ['--features'] or at('--features') != 'test-run-key':
                bad.append(f'{path} "{name}": enables more than test-run-key: {" ".join(flags)}')
            if at('--target-dir') != 'target-e2e':
                bad.append(f'{path} "{name}": does not build into --target-dir target-e2e')
            continue
        for word in flags:
            bad.append(f'{path} "{name}": {word}')
    if electron and E2E_BUILD not in scripts:
        bad.append(f'{path}: no {E2E_BUILD}, so no e2e run can turn a flag on')
for b in bad:
    print(b, file=sys.stderr)
sys.exit(1 if bad else 0)
PY
    printf 'arch/test_run_key FAIL: the packaging scripts (see above)\n' >&2
    failed=1
fi

if grep -nE 'cargo build.*(--all-features|--features|[[:space:]]-F)' .github/workflows/*.yml >&2; then
    printf 'arch/test_run_key FAIL: a workflow builds the backend with features\n' >&2
    failed=1
fi

# Both platforms' release backends are built in CI and searched for the
# key. Read with PyYAML: some job runs on windows-latest, builds the
# backend with electron's own `build:backend` (the script `package:win`
# runs), and has a later step that names the binary that build makes
# and the key's file. The Linux side is the release-shaped job's (`pnpm
# run dist`), whose steps name dist/linux-unpacked's backend the same
# way. What the steps do with those names is theirs; this holds that
# the jobs are there and point at the right binaries. It does not read
# whether they run or count: a job's or step's `if:` (every step here
# carries the filter's), `continue-on-error`, the workflow's triggers,
# or an `env` (RUSTFLAGS among them) that changes the build.
py=''
for candidate in python3 /usr/bin/python3; do
    if "$candidate" -I -c 'import yaml' 2>/dev/null; then
        py=$candidate
        break
    fi
done
if [ -z "$py" ]; then
    printf 'arch/test_run_key FAIL: no python3 with PyYAML to read the workflows (install python3-yaml)\n' >&2
    failed=1
elif ! "$py" -I - <<'PY'; then
import glob, sys
import yaml

KEY = 'backend/tests/fixtures/flags-signature/test-run-key.pub'
PLATFORMS = {
    'windows-latest': ('pnpm run build:backend', 'target/release/ani-gui-backend.exe'),
    'ubuntu-latest': ('pnpm run dist', 'dist/linux-unpacked/resources/ani-gui-backend'),
}


def checks(job, build, binary):
    steps = job.get('steps') or []
    for i, step in enumerate(steps):
        run = step.get('run')
        if isinstance(run, str) and run.strip() == build and step.get('working-directory') == 'electron':
            later = [str(t.get('run', '')) for t in steps[i + 1:]]
            return any(binary in r and KEY.split('backend/', 1)[1] in r for r in later)
    return False


found = set()
for path in sorted(glob.glob('.github/workflows/*.yml')):
    with open(path, encoding='utf-8') as f:
        doc = yaml.safe_load(f) or {}
    for job in (doc.get('jobs') or {}).values():
        for runner, (build, binary) in PLATFORMS.items():
            if job.get('runs-on') == runner and checks(job, build, binary):
                found.add(runner)
missing = sorted(set(PLATFORMS) - found)
for runner in missing:
    build, binary = PLATFORMS[runner]
    print(f'no {runner} job runs `{build}` in electron/ and then searches {binary} for the test-run key', file=sys.stderr)
sys.exit(1 if missing else 0)
PY
    printf 'arch/test_run_key FAIL: a release backend is not checked for the key in CI (see above)\n' >&2
    failed=1
fi

exit "$failed"
