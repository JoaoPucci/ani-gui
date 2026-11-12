#!/bin/sh
# Architectural invariant: no packaged backend trusts the test-run key.
#
# A backend built with its `test-run-key` cargo feature trusts a test
# run's flags file (ANI_GUI_TEST_FLAGS_FILE) under a fixture key whose
# secret anyone can derive from
# backend/tests/fixtures/flags-signature/make.py. The tests enable it
# (`cargo test --all-features`); a package must never carry it. That a
# build with the feature still fetches the live file under its own keys
# alone is the backend's tests' to hold (flags/launch_keys_test.rs).
# What this check holds is narrower than "no package can carry it":
#
#   - the feature is not among the backend's default features, directly
#     or through another feature `default` turns on, read from
#     backend/Cargo.toml with Python's TOML parser;
#   - no script in electron/package.json or the root package.json that
#     runs `cargo build` passes --features, -F or --all-features, read
#     with Python's JSON parser and split into words by its shell
#     lexer;
#   - no workflow line that runs `cargo build` passes one either.
#
# It does not see other routes to a feature build: RUSTFLAGS or a
# .cargo/config.toml setting `--cfg feature="test-run-key"`, other
# spellings of the build (`cargo b`, `cargo +toolchain build`,
# `cargo rustc`, `cargo install`, a command continued across lines), a
# script that builds through a helper file it calls, or a script that
# packages a binary some other command left behind (`dist:win` packages
# whatever target/release holds; `package:win` builds it first).
#
# Both platforms' packaging scripts are covered, Windows's
# `build:backend` among them. On Linux the e2e workflow also builds the
# package and checks that the binary does not hold the key ("The
# packaged backend trusts no test-run key"); nothing builds the Windows
# backend in CI, so on that side this check is all there is.

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
bad = []
for path in sys.argv[1:]:
    with open(path, encoding='utf-8') as f:
        scripts = json.load(f).get('scripts', {})
    for name, cmd in scripts.items():
        if 'cargo build' not in cmd:
            continue
        for word in shlex.split(cmd):
            if word in ('--all-features', '--features', '-F') or word.startswith(('--features=', '-F')):
                bad.append(f'{path} "{name}": {word}')
for b in bad:
    print(b, file=sys.stderr)
sys.exit(1 if bad else 0)
PY
    printf 'arch/test_run_key FAIL: a packaging script builds the backend with features\n' >&2
    failed=1
fi

if grep -nE 'cargo build.*(--all-features|--features|[[:space:]]-F)' .github/workflows/*.yml >&2; then
    printf 'arch/test_run_key FAIL: a workflow builds the backend with features\n' >&2
    failed=1
fi

exit "$failed"
