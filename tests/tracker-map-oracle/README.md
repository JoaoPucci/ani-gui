# The episode map's oracle

Two checks on the rules that file a Kitsu episode under a tracker's
entry (`backend/src/account/episode_map_rules.rs`, and `target` in
`episode_map.rs`):

- **Regression.** Every recorded case under
  `tests/fixtures/tracker-map/` — one per Kitsu entry whose count a
  tracker's entry disagrees with, found by comparing Kitsu's whole
  catalogue with AniList and MyAnimeList — is mapped, and the map is
  compared with `fixtures-golden.json`. Each case also names the shape
  the documented rules answer for it, and must map to it. Both are
  ordinary tests (`recorded_tracker_fixtures_map_as_the_golden_dump_records`,
  `every_recorded_mismatch_maps_to_the_shape_its_case_names`), so they
  run wherever the backend's tests run.
- **Agreement.** `oracle.py` is a model of the rules in
  [`docs/title-resolution.md`](../../docs/title-resolution.md)
  ("Kitsu's episodes on a tracker's entries"). It writes out every
  synthetic case it knows the docs' answer for — chains, shared
  entries, split parts, prologues and several half-episode recaps,
  conflicts, and the near misses of each — and every recorded case,
  each with the map and every episode's write it predicts. The app maps
  them, and the outcomes are compared. Any disagreement fails unless
  `allowlist.json` carries it with a reason. This runs as the
  `episode map oracle agreement` job of `.github/workflows/rust.yml`
  whenever a change touches the backend, the recorded cases, this
  directory, the document the model reads, or the workflow.

Nothing here reads the app's source. When the app and the model
disagree, one of them is wrong about the document.

## Running it

From the repository root, with `python3` and the Rust toolchain:

```sh
# The model against the docs' table
python3 -I -m unittest discover -s tests/tracker-map-oracle -p 'test_*.py'

# The app against the model: generate, run, compare
python3 -I tests/tracker-map-oracle/oracle.py gen /tmp/tracker-map-cases.json
(cd backend && TRACKER_MAP_ORACLE_CASES=/tmp/tracker-map-cases.json \
    TRACKER_MAP_ORACLE_OUT=/tmp/tracker-map-out.json \
    cargo test --lib -- --ignored --exact \
    account::episode_map_rules::oracle::run_the_tracker_map_oracle_cases)
python3 -I tests/tracker-map-oracle/oracle.py cmp \
    /tmp/tracker-map-cases.json /tmp/tracker-map-out.json \
    --allowlist tests/tracker-map-oracle/allowlist.json

# Regenerate the golden, in a change that means to alter a recorded map
(cd backend && cargo test --lib write_the_tracker_fixture_golden -- --ignored)
```

The generated set is a deterministic function of `oracle.py` and the
recorded cases, so it is not committed.

## What a disagreement means

As for the picker's oracle: the model misread the docs (fix the
model), the docs are wrong or silent (fix both together — the model
follows the docs, never the app), or the app departs from the docs
(fix the app, a recorded case first; or allowlist the case with its
outcome and a reason, in the change that accepts the departure). The
golden and the allowlist change only in a change that changes the
rules, with the reason in the commit body.

## The rule for a reported mismatch

Every reported tracker mismatch — a show whose progress lands on the
wrong entry, under the wrong number, or completes at the wrong episode
— becomes a recorded case in `anilist.json` or `mal.json`: the
evidence as the app's readers return it, the shape the docs answer,
and a note where it is pinned. The golden then records its map.
