#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC2312 — command substitutions are read for their text, and a
#       failure arrives as an empty result that the assertion then catches
#
# Scoped to this file rather than widened in SHELLCHECK_OPTS, which
# would also relax the checks guarding the `ani-cli` script itself.
# shellcheck disable=SC2312
# Architectural invariant: the backend never returns localized strings.
# Errors are stable i18n keys (`error.scraper.timeout`) — the frontend
# resolves them.
#
# Concrete checks:
#   1. AniError variants in src/error.rs all have an entry in key().
#   2. Every constant in src/i18n.rs starts with "error." and has at
#      least three dot-separated segments.
#   3. (Future, when frontend lands) No hardcoded English text in
#      .svelte files except inside aria-* / data-testid attributes.
#
# Most of the actual checking lives inside the Rust unit tests in
# error.rs and i18n.rs (every_variant_has_a_stable_key,
# every_key_is_well_formed). This script is a fast grep-level
# canary so a contributor running `bash tests/arch/run-all.sh` gets a
# quick "yes, the i18n discipline is on" signal without needing the
# Rust toolchain.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -f backend/src/error.rs ]; then
    printf 'arch/i18n: error.rs not present yet — skipping\n'
    exit 0
fi

failed=0

# 1. error.rs must contain a `fn key(&self) -> &'static str` with arms
# for every variant. Variant names are pulled from the enum AniError
# definition; every name should appear at least twice (once in the
# enum, once in the key() match).
variants=$(awk '
    /^pub enum AniError/ { in_enum = 1; next }
    in_enum && /^}/ { in_enum = 0 }
    in_enum {
        # Match `Variant {` or `Variant,` or `Variant\n` after stripping
        # leading whitespace.
        gsub(/^[[:space:]]+/, "")
        if ($0 ~ /^[A-Z][A-Za-z0-9]*[[:space:]]*[\{,]/) {
            sub(/[\{,].*$/, "")
            gsub(/[[:space:]]/, "")
            if ($0 != "") print $0
        }
    }
' backend/src/error.rs)

for v in $variants; do
    # Must appear in key() match (Self::Variant ... => "...")
    if ! grep -q "Self::$v" backend/src/error.rs; then
        printf 'arch/i18n FAIL: AniError::%s has no arm in key()\n' "$v" >&2
        failed=1
    fi
done

# 2. i18n.rs constants must be well-formed.
if [ -f backend/src/i18n.rs ]; then
    while IFS= read -r line; do
        # Match e.g.: pub const NAME: &str = "error.scope.thing";
        value=$(printf '%s\n' "$line" | sed -nE 's/.*"([^"]*)".*/\1/p')
        [ -z "$value" ] && continue
        case "$value" in
            error.*.*) ;; # ok
            *)
                printf 'arch/i18n FAIL: i18n constant value %s does not match error.scope.name\n' "$value" >&2
                failed=1
                ;;
        esac
    done <<EOF
$(grep -E '^pub const [A-Z_]+: &str' backend/src/i18n.rs)
EOF
fi

# 3. No user-facing string names the CLI's history file.
#
# The GUI keeps its own watch history at `<state_dir>/history`
# (`config::paths::gui_history`). It shared the CLI's `ani-hsts` in the
# allanime era and does not anymore, so a message telling a user to
# "populate ani-hsts" sends them to a file this app never writes — and
# four locales said exactly that, long after the code moved.
#
# Scoped to the message bundles because there the question is
# mechanical: those files are nothing but text shown to users, so any
# mention is a claim made to one. Nowhere else is that true. The same
# name in `paths.rs` is an accurate note about what the GUI used to do
# — telling that from a stale claim means reading for meaning, which
# is the one thing an architectural check here may not do.
#
# So: comments and prose outside the bundles are NOT covered by this
# check, and a green run says nothing about them.
if [ -d frontend/messages ]; then
    hits=$(grep -rl 'ani-hsts' frontend/messages 2>/dev/null || true)
    if [ -n "$hits" ]; then
        printf 'arch/i18n FAIL: user-facing message names the CLI history file (the GUI keeps its own at <state_dir>/history):\n%s\n' "$hits" >&2
        failed=1
    fi
fi

if [ "$failed" -eq 0 ]; then
    printf 'arch/i18n PASS (variants checked: %s)\n' "$(printf '%s\n' "$variants" | wc -l | tr -d ' ')"
fi
exit "$failed"
