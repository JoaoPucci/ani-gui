#!/bin/sh
# Architectural invariant: every feature tip the backend declares
# (backend/src/onboarding/tips.json) points at places the renderer has,
# and has its words in every locale.
#
# For each declared tip, read with Python's JSON parser:
#
#   - each of its places is marked `data-onboard="<place>"` exactly once
#     across the .svelte files under frontend/src. This counts marks in
#     the source, not elements on screen: a component rendered several
#     times renders its mark several times, and the renderer then lights
#     the first. Whether a mark renders once is not seen here;
#   - each of its steps has `<id>_<n>_title` and `<id>_<n>_body` (the
#     id with `-` as `_`, the step from 1) in every locale's
#     frontend/messages/<locale>/onboarding.json, the locales being those
#     frontend/project.inlang/settings.json lists. Paraglide names them
#     `onboarding_<id>_<n>_title` and `…_body`.
#
# Syntactic constraints, not interpretation: the mark is counted as
# text, so a mark inside a comment counts too, and a place marked by an
# expression (`data-onboard={x}`) is not seen. Whether the renderer's
# words table ($lib/onboarding/words.ts) names each step is its own
# test's to hold (words.test.ts), and whether the place is on screen
# when the tip would show is the end-to-end suite's.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

exec python3 -I - "$REPO_ROOT" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
tips = json.loads((root / "backend/src/onboarding/tips.json").read_text(encoding="utf-8"))
locales = json.loads((root / "frontend/project.inlang/settings.json").read_text(encoding="utf-8"))["locales"]
sources = [
    p.read_text(encoding="utf-8")
    for p in sorted((root / "frontend/src").rglob("*.svelte"))
]
failed = False


def fail(msg):
    global failed
    failed = True
    print(f"arch/onboarding_places FAIL: {msg}", file=sys.stderr)


words = {}
for loc in locales:
    path = root / "frontend/messages" / loc / "onboarding.json"
    try:
        words[loc] = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        words[loc] = {}
        fail(f"{path.relative_to(root)} does not exist")

for tip in tips:
    tid = tip["id"]
    for place in tip["places"]:
        mark = f'data-onboard="{place}"'
        count = sum(src.count(mark) for src in sources)
        if count != 1:
            fail(f"{tid}: {mark} appears {count} times in frontend/src/**/*.svelte, not once")
    stem = tid.replace("-", "_")
    for n in range(1, len(tip["places"]) + 1):
        for part in ("title", "body"):
            key = f"{stem}_{n}_{part}"
            for loc in locales:
                if not str(words[loc].get(key, "")).strip():
                    fail(f"{tid}: step {n} has no {key} in {loc}/onboarding.json")

if not failed:
    print(f"arch/onboarding_places: {len(tips)} tips, places and words in step")
sys.exit(1 if failed else 0)
PY
