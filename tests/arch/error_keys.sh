#!/bin/sh
# Architectural invariant: every error key the backend can answer with
# has a message in every shipped locale, and no locale carries a
# message for a key the backend cannot answer with.
#
# The subject is `backend/error-keys.json`, the key list the backend
# publishes. `cargo test` holds that file to the `AniError` variants
# through a list a new variant does not compile past
# (`backend/src/error_variants_test.rs`), so this check reads a list
# the backend vouches for rather than inferring one from Rust source.
# Each locale in `frontend/project.inlang/settings.json` carries
# `frontend/messages/<locale>/backend.json`, and the renderer reaches a
# key's message by the key with its dots as underscores
# (`error.metadata.source` → `error_metadata_source`), so the invariant
# is set equality under that one rewrite, per locale. A key with no
# message in one locale shows that locale the English text — the
# compile falls back to it — and with none in any locale the kind's
# coarser sentence; a message with no key is copy nothing can reach;
# an empty message is a failure the screen says nothing about. The
# rewrite must also be one-to-one over the list: two keys reaching one
# name would share a message that is right for at most one of them.
#
# Every input is JSON and is read with a JSON parser. What this does
# NOT check, and a green run says nothing about: that the published
# list matches the Rust source (the Rust test's job — a change to
# either file runs it in CI), and that the renderer's table in
# `describe-error.ts` has an entry per key (`describe-error.test.ts`
# holds that against the same list; an entry for a key the list lacks
# names a message this check rejects, and the compile then fails).

set -eu

REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$REPO_ROOT"

if [ ! -f backend/error-keys.json ]; then
    printf 'arch/error_keys: backend/error-keys.json not present — skipping\n'
    exit 0
fi

node - <<'EOF'
const fs = require('node:fs');
const path = require('node:path');

const problems = [];

/** The parsed file, or undefined with the reason recorded. */
function read(file) {
    try {
        return JSON.parse(fs.readFileSync(file, 'utf-8'));
    } catch (e) {
        problems.push(`${file}: ${e.message}`);
        return undefined;
    }
}

const isStringList = (v) =>
    Array.isArray(v) && v.length > 0 && v.every((x) => typeof x === 'string');

const KEYS = path.join('backend', 'error-keys.json');
const SETTINGS = path.join('frontend', 'project.inlang', 'settings.json');

const keys = read(KEYS);
if (keys !== undefined && !isStringList(keys)) {
    problems.push(`${KEYS}: not a non-empty list of strings`);
}
const settings = read(SETTINGS);
const locales = settings !== null && typeof settings === 'object' ? settings.locales : undefined;
if (settings !== undefined && !isStringList(locales)) {
    problems.push(`${SETTINGS}: "locales" is not a non-empty list of strings`);
}

if (problems.length === 0) {
    // Message name → the key it answers; the rewrite must be one-to-one.
    const keyOf = new Map();
    for (const key of keys) {
        const name = key.replace(/\./g, '_');
        if (keyOf.has(name)) {
            problems.push(
                keyOf.get(name) === key
                    ? `${KEYS}: ${key} is listed twice`
                    : `${KEYS}: ${keyOf.get(name)} and ${key} both reach the message name ${name}`
            );
            continue;
        }
        keyOf.set(name, key);
    }
    for (const locale of locales) {
        const file = path.join('frontend', 'messages', locale, 'backend.json');
        const texts = read(file);
        if (texts === undefined) continue;
        if (texts === null || typeof texts !== 'object' || Array.isArray(texts)) {
            problems.push(`${file}: not an object of messages`);
            continue;
        }
        for (const [name, key] of keyOf) {
            if (!Object.hasOwn(texts, name)) {
                problems.push(`${file}: no message for ${key}`);
            } else if (typeof texts[name] !== 'string' || texts[name].trim() === '') {
                problems.push(`${file}: the message for ${key} is empty`);
            }
        }
        for (const name of Object.keys(texts)) {
            if (!keyOf.has(name)) {
                problems.push(`${file}: message "${name}" answers no backend key`);
            }
        }
    }
}

if (problems.length > 0) {
    console.error("arch/error_keys FAIL: the backend's error keys and the locale catalogues disagree:");
    for (const p of problems) console.error(`  ${p}`);
    console.error(
        'Every key in backend/error-keys.json needs one message per locale in frontend/messages/<locale>/backend.json, named by the key with its dots as underscores, and nothing else may be in that file.'
    );
    process.exit(1);
}
console.log(`arch/error_keys PASS (keys: ${keys.length}, locales: ${locales.length})`);
EOF
