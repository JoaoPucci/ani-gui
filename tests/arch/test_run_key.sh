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
#     included. So no release script reaches the feature build, or
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
# manager or `run`, or a script that packages a binary some other
# command left behind
# (`dist:win` packages whatever target/release holds; `package:win`
# builds it first).
#
# Both platforms' packaging scripts are covered, Windows's
# `build:backend` among them. On Linux the e2e workflow also builds the
# release-shaped package in a job of its own and checks that its binary
# does not hold the key ("The release-shaped backend trusts no test-run
# key"), and that the e2e package's binary does, so the check is seen
# to find it; nothing builds the Windows backend in CI, so on that side
# this check is all there is.

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

exit "$failed"
