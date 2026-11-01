# The season picker's oracle

Two checks on the picker that chooses a provider listing for a Kitsu
entry (`backend/src/commands/play_native_walk.rs` and its siblings):

- **Regression.** Every recorded pool under `tests/fixtures/hianime/picker/`
  is walked and its outcome — the slug picked, the regular episode
  count, every episode's id, number and tag, or the refusal's error and
  clean-miss verdict — is compared with `fixtures-golden.json`. A
  recorded show whose pick changes fails `cargo test` until the golden
  is regenerated. This is an ordinary test, so it runs wherever the
  backend's tests run.
- **Agreement.** `oracle.py` is a model of the decision tables in
  [`docs/title-resolution.md`](../../docs/title-resolution.md). It
  writes out every synthetic pool it knows the docs' answer for, the
  real picker is run over them, and the outcomes are compared with the
  model's. Any disagreement fails unless `allowlist.json` carries it
  with a reason. This runs as the `picker oracle agreement` job of
  `.github/workflows/rust.yml` whenever a change touches the picker,
  its fixtures, this directory or the document the model reads.
- **Differential fuzzing.** `oracle.py fuzz` draws pools from a grammar
  over the docs' vocabulary, each with the pick the model predicts, and
  the same driver and `cmp` run them. The generated set walks the
  shapes the docs' tables name; the fuzzer joins the words those tables
  are written in every way a catalogue writes them, which is where hand
  probes kept finding the next disagreement. It runs as the `picker
  oracle fuzz` job beside the agreement job, on the same changes, with
  the run number as its seed.

Nothing here reads the picker's source. When the picker and the model
disagree, one of them is wrong about the document, and the comparison
says which pool.

## What lives here

| File | What it is |
|---|---|
| `oracle.py` | The model and the comparison. Standard library only. |
| `fuzz.py` | The fuzzer's grammar: names in Latin, accented and Japanese script, numbers in ASCII and full-width digits, Roman numerals and spelled ordinals, the division words, catalogue and version tags in every bracket pair and width, punctuation, and words glued with no space. |
| `test_fuzz.py` | The fuzzer's own tests: a seed decides its pools, every pool carries the model's prediction, and a sample of each part of the grammar's vocabulary is reached. |
| `test_oracle.py` | The model's own tests: rows transcribed from the docs' tables predict as the tables say; the generated set is reproducible; the comparison refuses what it must. |
| `cases/*.json` | Hand-written pools, each with the pick the docs answer. `doc-table.json` is the docs' pool table row by row, but for three rows the driver cannot express (below); `fuzz-found.json` is every shape the fuzzer found, reduced by hand to the rows that decide it and pinned once it was resolved — one, `fuzz/beside-repeated-season`, is a broad-listing pool the model tables, so its expectation is written from the docs' table rather than predicted; the others are the shapes that surfaced while reviewing picker changes. |
| `alphanumeric.txt` | The characters Rust's `char::is_alphanumeric` accepts, as ranges: the letters and digits the picker splits a title's words on, which the model reads instead of Python's narrower `str.isalnum`. A driver test holds it to the pinned toolchain and prints the table to write when a bump moves it. |
| `allowlist.json` | Disagreements accepted for now, each with its reason and the outcome it accepts. |
| `fixtures-golden.json` | Every recorded fixture's outcome, one show per line. |

The driver is `backend/src/commands/play_native_walk_oracle_test.rs`,
a test module beside the fixture tests, since it needs their recording
seam. It carries the golden comparison, the two ignored tests the
commands below run, and the tests of its own output format.

## Running it

From the repository root, with `python3` (3.12 or later) and the Rust
toolchain:

```sh
# The model against the docs' tables (about five minutes)
python3 -I -m unittest discover -s tests/picker-oracle -p 'test_*.py'

# The picker against the model: generate, run, compare
python3 -I tests/picker-oracle/oracle.py gen /tmp/picker-oracle-cases.json
(cd backend && PICKER_ORACLE_CASES=/tmp/picker-oracle-cases.json \
    PICKER_ORACLE_OUT=/tmp/picker-oracle-out.json \
    cargo test --lib run_the_oracle_cases -- --ignored)
python3 -I tests/picker-oracle/oracle.py cmp \
    /tmp/picker-oracle-cases.json /tmp/picker-oracle-out.json \
    --allowlist tests/picker-oracle/allowlist.json

# The picker against the model on pools the fuzzer draws (about two
# minutes for 80000; CI runs 20000 from its run number)
python3 -I tests/picker-oracle/oracle.py fuzz 1 80000 /tmp/picker-oracle-fuzz.json
(cd backend && PICKER_ORACLE_CASES=/tmp/picker-oracle-fuzz.json \
    PICKER_ORACLE_OUT=/tmp/picker-oracle-fuzz-out.json \
    cargo test --lib run_the_oracle_cases -- --ignored)
python3 -I tests/picker-oracle/oracle.py cmp \
    /tmp/picker-oracle-fuzz.json /tmp/picker-oracle-fuzz-out.json

# The recorded fixtures against the golden (part of `cargo test`)
(cd backend && cargo test --lib recorded_fixtures_pick_as_the_golden_dump_records)

# Regenerate the golden, in a change that means to alter a recorded pick
(cd backend && cargo test --lib write_the_fixture_golden -- --ignored)
```

Generating the set takes about twenty seconds and writes some 65,000
pools (18 MB); the picker walks them in about thirty-five seconds in the
debug profile. The generated set is not committed: it is a deterministic
function of `oracle.py` and `cases/`, so committing it would store the
generator's output beside the generator. The whole set runs on every
relevant pull request.

The fuzzer is separate from the generated set and leaves it as it is.
A seed and a count decide its pools, byte for byte, at one commit of
`fuzz.py` and `oracle.py` — a pool the model does not compute is drawn
again, so a change to the model can change which pools a seed writes —
and a longer run from a seed extends a shorter one, so a pool a CI run
reports is drawn again from the seed and commit the job prints. The
fuzz job compares without the allowlist: an entry names a pool of the
generated set by its id, and a fuzz run's pools are other pools, so a
disagreement the fuzzer finds is resolved, never accepted. A pool the model does not compute
(`NOT_MODELLED`, below) is drawn again rather than written. Every
disagreement it finds is resolved as any other is (below) — the picker
fixed against the docs, or the model corrected, or the docs settled
and both made to follow — and the shape goes into
`cases/fuzz-found.json`, so it is pinned in the generated set from
then on. The docs' "Readings at the grammar's edges" paragraph states
the rules those shapes settled.

`cmp` prints a summary line, one line per family and then every
problem: a pool whose outcome differs from the model and is not
allowlisted — a pick by its slug, count and every episode it lists, a
refusal by its error and clean-miss verdict — an allowlisted pool whose
outcome moved from the one recorded, an allowlist entry gone stale (its
pool agrees now), one naming no pool, or one giving no reason, a pool
the walk panicked on, a pool with no outcome. It exits non-zero on any
of them.

## What a disagreement means

The model is a reading of the document, so a disagreement has three
possible causes, and the first thing to do is decide which:

1. **The model misread the docs.** Fix the model, and say so in the
   commit. This is the common case for a rule the docs gained after
   the model was written.
2. **The docs are wrong or do not decide the case.** Fix the document
   and the model together — the model follows the docs, never the
   picker.
3. **The picker departs from the docs.** Either fix the picker — a
   failing case first: for a reported mispick, its recorded fixture,
   per the rule below; for a shape the generated set or the fuzzer
   found, a case in `cases/` (`fuzz-found.json` for the fuzzer's) —
   or, when the departure is accepted for now, add an allowlist entry
   in the same change:

   ```json
   {"id": "countless/0123456789",
    "got": {"pick": "x", "count": 2, "episodes": [[1, 1, null], [2, 2, null]]},
    "reason": "why the picker is allowed to differ here"}
   ```

   `id` is the pool's id as `cmp` prints it; `got` is the outcome the
   entry accepts, a pick as `{"pick", "count", "episodes"}` or a refusal
   as `{"refused", "clean_miss"}`, so a later change that moves the
   outcome again is caught; `reason` is for the next reader. A
   generated pool's id follows its content, not its position, so the
   entry survives the generator growing.

The allowlist and the golden change only in a change that changes the
picker, with the reason in the commit body. A pull request that touches
either without touching the picker is asking the checker to look away.

## The rule for a reported mispick

Every reported mispick becomes a recorded fixture under
`tests/fixtures/hianime/picker/` — the entry's Kitsu arguments and
every search, listing and entry page the walk read for it, reduced to
what the parsers return (`play_native_walk_record_test.rs` records one)
— with a `MANIFEST.json` entry and an expectation in one of the fixture
test's tables in `play_native_walk_fixture_test.rs` — a pick or a clean
miss in an `Expectation` table (`AUDITED_RIGHT`,
`UNDATED_AUDITED_RIGHT`), a refusal that is not the clean miss in
`UNDATED_REFUSED` or `DATED_REFUSED` — in the pull request that fixes
it. That recording then holds the fix: the golden
records its outcome, and the fixture test asserts its pick.

A reported mispick is a real show the app picked wrong, and its
recording is what the provider actually sent. A shape the fuzzer finds
on a synthetic pool is not one: there is no provider response to
record, and a fixture made up for it would misrepresent what the
provider sends. It is pinned as a case in `cases/fuzz-found.json`
instead, which the generated set walks in CI; where the recorded
corpus already holds a real title of the same shape, that recording's
golden and table expectation hold it too. Neither stands in for the
other: a reported mispick is recorded even when a case already covers
its shape, and a fuzz-found shape is pinned as a case even when no real
listing shows it.

## What the model computes, and what it tables

The predicates compute admission (the title rule), the entry before,
the season before, the sequel guard, the probe head, the pick by count
and its versions when no listing is broader than the entry, and the countless order
through the one reader that decides whether the entry names a hit.
Pools the predicates do not compute — a broad listing a sibling could
complete (the spanning cut), a bare title beside parts of it whose
counts could together explain the entry's (stitching), a dead listing,
a one-video entry — are tabled: `gen()` writes their expectations row
by row from the decision tables, and `cases/doc-table.json` carries
the table itself — all but five rows, which the driver cannot express
because it names 2020 on every page, badges every hit TV and lists the
same genre, never Hentai, on every page: the dated "Show: Arc" whose hit's page names no year, pinned by
the fixture test `an_entry_with_a_count_takes_only_a_hit_named_for_it`;
Tsubu★Doll, whose refusal rests on a format badge and is pinned by its
recording (8404, in `UNDATED_REFUSED`); and Ohanashi de Te Koi given
its hentai's year and title, and again undated with its title alone,
whose refusals rest on the hit's genre and are pinned by
`a_hit_filed_under_hentai_is_never_picked`; and the undated "Show: Arc"
whose listing's page lists no genre, pinned by
`an_undated_pick_never_takes_an_unread_page_as_safe` and
`a_refusal_beside_an_unread_page_is_the_answered_dead_end`. `predict()` answers `NOT_MODELLED` for those shapes,
and the model's tests assert that where both a predicate and a tabled
row speak, they agree.

Every generated listing numbers its episodes from 1 with no tag and its
page names 2020, the year a generated entry carries when it carries one
(`LISTING_YEAR`, in `oracle.py` and in the driver), and every search
card carries the entry's canonical title as its romanized name unless
the pool says otherwise (`card_names` in `oracle.py`, `site_of` in the
driver), as the provider's cards for an entry's own listings carry the
entry's romaji — so a name vouches for every hit, as the year does, and
the titles alone decide. The `counted-dated` family is the one that
says otherwise: its pools name nothing on their cards, or name an
unrelated hit that fits the count on its card or its page, so the rule
that a dated entry with a count takes only a hit a name vouches for is
walked. The model does not reach
what the year filter or the format badge does on a real pool; the
recorded fixtures do.

## Case format

```json
{"id": "kanji/1", "titles": ["ショー 第十期"], "count": 12, "entry_year": 2020,
 "rows": [["x", "Show Season 10", 12]],
 "expect": {"pick": "x", "count": 12}, "family": "kanji"}
```

`titles` is the entry's canonical title first, then its aliases; every
title's search answers with the same `rows` of `(slug, title,
episodes)`, in the provider's order. `count` and `entry_year` are
Kitsu's, either `null` when unknown. `expect` is the pick the docs
answer — the slug and its regular count — or `null` for a pool the walk
must refuse. A pick is held to the first `count` rows of its listing, so
a case cannot expect a stitched pick, whose rows keep each part's ids. An optional `dead` lists slugs whose probe dies in
transport. An optional `jnames` maps a slug to the romanized name its
search card carries — given, it replaces the convention above, and a
slug it leaves out carries none — and an optional `page_names` maps a
slug to the names its entry page gives. Hand-written ids are given by hand; generated ones are the
family and a hash of the pool.
