# Testing

This project is **strictly TDD**. Every code change starts with a failing test.

## The pyramid

```
         ┌─────────────────────┐
         │   Mutation (deferred)│  cargo-mutants + stryker, nightly,
         └─────────────────────┘  not gating
       ┌───────────────────────────┐
       │ Architectural invariants  │  tests/arch/*.sh + their bats
       └───────────────────────────┘  harness — i18n, deps, deferrals
     ┌─────────────────────────────────┐
     │           Property              │  proptest (Rust),
     └─────────────────────────────────┘  fast-check (TS)
   ┌──────────────────────────────────────┐
   │              End-to-end              │  Playwright (planned),
   └──────────────────────────────────────┘  ≤5 hermetic scenarios
 ┌──────────────────────────────────────────┐
 │              Acceptance                  │  cargo integration,
 └──────────────────────────────────────────┘  vitest + MSW
┌──────────────────────────────────────────────┐
│                  Unit                        │  cargo test, vitest,
└──────────────────────────────────────────────┘  colocated *.test.ts
```

## Test layout

```
tests/
├── bash/                  # bats harness for the arch checks
│   ├── helpers/           # loader, vendored-bats runner
│   └── arch/              # drives tests/arch/ through its cases
├── fixtures/              # shared goldens (bash + rust + ts)
│   ├── kitsu/             # JSON:API responses
│   ├── history/           # watch-history samples
│   ├── hls/               # transport streams as base64 text, a master playlist
│   └── arch/              # stand-in checks the bats harness drives
├── picker-oracle/         # the season picker's checks (README inside)
│   ├── oracle.py          # model of docs/title-resolution.md, and the comparison
│   ├── fuzz.py            # the differential fuzzer's grammar of titles
│   ├── cases/             # hand-written pools with the docs' answers
│   ├── allowlist.json     # accepted disagreements, each with its reason
│   └── fixtures-golden.json  # every recorded picker fixture's outcome
├── tracker-map-oracle/    # the episode map's checks (README inside)
│   ├── oracle.py          # model of the tracker-entry rules, and the comparison
│   ├── allowlist.json     # accepted disagreements, each with its reason
│   └── fixtures-golden.json  # every recorded tracker case's map
└── arch/                  # cross-cutting architectural invariants
    ├── i18n.sh
    ├── deferral_record.sh
    ├── linux_deps.sh / windows_deps.sh
    └── run-all.sh         # executes every check

backend/
├── src/                   # #[cfg(test)] mod tests inline
├── tests/                 # cargo integration tests (acceptance)
└── proptests/             # proptest-only suites

frontend/
├── src/                   # *.test.ts colocated with units
├── tests/acceptance/      # vitest + MSW
├── tests/acceptance-guard/ # the acceptance guard over fixture scenarios; runs in `pnpm test`
└── e2e/                   # Playwright (planned)
```

## Running tests locally

```sh
# Bash (the arch harness)
tests/bash/helpers/install-bats.sh    # one-time, pins bats-core + plugins
tests/bash/helpers/run-suite.sh

# Rust backend
cd backend && cargo test --workspace
cd backend && cargo test --test proptests
# ... and as CI's Unix legs run it, under a temporary directory whose
# name a shell, a TOML string or a printf format reads specially
cd backend && TMPDIR="$(sh ../tools/hostile-tmpdir.sh "${TMPDIR:-/tmp}")" cargo test --workspace

# Frontend
cd frontend && pnpm test
cd frontend && pnpm test:acceptance

# Architectural invariants (always fast; the workflow-certification
# self-test parses CI config with PyYAML — python3-yaml on Debian/Ubuntu)
bash tests/arch/run-all.sh

# The season picker against the model of its docs (tests/picker-oracle/README.md)
python3 -I -m unittest discover -s tests/picker-oracle -p 'test_*.py'
```

## Coverage targets

Layer-specific. CI fails on regression below the baseline in `coverage-baseline.json`, not on absolute floors.

The CRAP ceilings (`crap.*` in `coverage-baseline.json`) are a separate, firm gate. A PR that would push a file's CRAP score above `max_le`, or push the count of high-risk files above `high_risk_le`, must refactor (split files, extract helpers, cover more) to bring the code under the ceiling — raising the ceiling to fit new code is not allowed. The tool deliberately leaves `crap.*` untouched on `--update` so the policy is enforced mechanically, not just by reading discipline. A separate `--update-crap` opt-in lets a deliberate cleanup *tighten* the CRAP ceilings (it refuses to write any value looser than the current baseline).

The complexity side of CRAP comes from a parser per language: `tools/rust-ccn` (on `syn`) for `backend/src` and `tools/ts-ccn.mjs` (on the TypeScript compiler) for `frontend/src`. Each counts every function its parser finds — closures in Rust and arrow functions in TypeScript included, each a unit of its own, while a Rust async block is charged to the function that evaluates it — at 1 plus one per decision — `if`, `for`, `while`, `&&`, `||` in both languages; `match`, `where` and `?` in Rust; `case`, `catch`, the conditional `? :`, `&&=` and `||=` in TypeScript — and fails the job on a file it cannot parse, or on a root that does not exist or holds nothing to measure. What they do not measure, they print in the job log: Rust functions written inside an item-position macro such as `proptest!` are counted as that macro's block rather than one by one, and a closure written inside macro input (`assert!`, `tokio::select!`) counts for the unit around it, since macro input is not parsed (there the pipes of an empty closure still read as a `||` decision); decisions outside every function (the `for` of `impl Trait for Type`, module-scope code) are counted and charged to no file. Every file under a root is measured, listed as not measured, or fails the job. `ts-ccn` measures every script kind the compiler reads (`.ts`, `.mts`, `.cts`, `.tsx`, `.js`, `.mjs`, `.cjs`, `.jsx`); the coverage run instruments only `src/lib/**/*.ts`, so a measured production file outside that glob scores at zero coverage (test files are left out of the ranking). It lists `.svelte` components, declaration files, and `.html` (without an inline `<script>`), `.css` and `.json` as not measured, and counts the generated Paraglide output it skips (`src/lib/paraglide`, the compiler's output directory, and nothing else). `rust-ccn` measures `.rs` and lists `.sql`, `.json`, `.toml`, `.txt` and `.md` as holding no Rust. A file of any other kind fails the job until its kind is measured or declared in the tool. To reproduce a run locally, generate both lcov files (`sh tools/rust-lcov.sh` from the repository root, which writes `backend/lcov.info`, and `pnpm run test:coverage` in `frontend/`), then run the commands of the "Compute CRAP" step in `.github/workflows/crap.yml` from the repository root. Both coverage runs generate the same property inputs every time: `tools/rust-lcov.sh` pins proptest's seed (`PROPTEST_RNG_SEED`), and the frontend's coverage run hands fast-check a fixed seed (`FAST_CHECK_SEED`, from `vite.config.ts`, applied by `tests/property-seed.ts`), so a file's coverage, and with it whether it crosses the high-risk bar, does not change with the property seed (inputs that depend on something else, such as a HashMap's iteration order, the clock or a test that reaches the network, can still move it); `tests/arch/coverage_seed.sh` keeps the pins in place. The ordinary test runs keep a fresh seed. The toolchain is the one `rust-toolchain.toml` pins, for CI's coverage jobs as for a local run: the workflows install stable, and rustup takes the pinned toolchain over it in the repository. The scorer's table lists the worst files and every file within five of the high-risk bar, so the ones a sliver of coverage would tip over are read from there, in the build log or locally, rather than kept anywhere by hand.

The percentage baselines (`rust.*`, `frontend.*`) are tighten-only by the same mechanism: `node tools/check-coverage-baseline.mjs --update` will *raise* a floor when the measurement is higher than the baseline, but refuses to lower it. A PR that drops coverage has to restore it (or write the missing tests at a different layer) rather than re-baseline. The CRAP rule says "you can't loosen the ceiling for new code"; this is the symmetric rule for the floor.

| Layer | Tool | Line | Branch |
|---|---|---|---|
| Rust core (proxy, cache, scraper, history) | `cargo llvm-cov` | 85% | 75% |
| Rust glue (HTTP API handlers) | `cargo llvm-cov` | 60% | — |
| Frontend lib/stores | vitest + c8 | 80% | — |
| Frontend components | vitest + c8 | 50% | — |
| E2E | scenario count | ≥5 | — |

## CI gates

Every PR runs all gating workflows; merge blocks until they're green:

| Workflow | Triggers | Gating |
|---|---|---|
| `bash.yml` | PR touches `tests/bash/**`, `tests/arch/**`, a script the suites execute, or a workflow | yes |
| `rust.yml` | PR touches `backend/**`, `tests/fixtures/**`, `rust-toolchain.toml`, `tests/picker-oracle/fixtures-golden.json`, `tests/tracker-map-oracle/fixtures-golden.json`, `tools/hostile-tmpdir.sh`, `tools/rust-lcov.sh` or the workflow — decided by `tools/rust-workflow-filter.sh`, which `tests/bash/arch/rust_workflow_filter.bats` runs | yes |
| `rust.yml`, job `picker oracle agreement` | PR touches `backend/**`, `tests/fixtures/hianime/picker/**`, `tests/picker-oracle/**`, `docs/title-resolution.md` or the workflow — the same script | yes |
| `rust.yml`, job `picker oracle fuzz` | the same changes as `picker oracle agreement`; draws 20000 pools from the run number as its seed and prints the seed | yes |
| `rust.yml`, job `episode map oracle agreement` | PR touches `backend/**`, `tests/fixtures/tracker-map/**`, `tests/tracker-map-oracle/**`, `docs/title-resolution.md` or the workflow — the same script | not in the ruleset's required list yet |
| `frontend.yml` | PR touches `frontend/**` | yes |
| `arch.yml` | always | yes |
| `mutation.yml` | nightly cron + manual dispatch | no (informational) |

## The season picker

The picker that chooses a provider listing for a Kitsu entry is checked
two ways, both under `tests/picker-oracle/` (its README has the
commands): every recorded pool under `tests/fixtures/hianime/picker/`
must pick as `fixtures-golden.json` records, an ordinary `cargo test`;
and the picker must agree with `oracle.py`, a model of the decision
tables in `docs/title-resolution.md`, on every pool the model writes
out, unless `allowlist.json` carries the disagreement with a reason.
A differential fuzzer beside it (`oracle.py fuzz`) draws further pools
from a grammar of the docs' vocabulary, seeded, and holds the picker to
the model's prediction on each; every shape it has found is pinned in
`tests/picker-oracle/cases/fuzz-found.json`.

The rule for a reported mispick: it becomes a recorded fixture under
`tests/fixtures/hianime/picker/` — with a `MANIFEST.json` entry and an
expectation in the fixture test's table — in the PR that fixes it. A
shape the fuzzer finds on a synthetic pool is no reported mispick and
has no provider response to record: it is pinned as a case in
`tests/picker-oracle/cases/fuzz-found.json`. Neither replaces the
other; the oracle's README has the full rule. The
golden and the allowlist change only in a PR that changes the picker,
with the reason in the commit body. The model follows the docs: when
the model and the docs disagree, fix whichever is wrong and say which;
it is never bent to the picker.

## The episode map

The rules that file a Kitsu episode under a tracker's entry are checked
the same two ways, under `tests/tracker-map-oracle/` (its README has
the commands): every recorded case under `tests/fixtures/tracker-map/`
must map to the shape it names and as `fixtures-golden.json` records,
an ordinary `cargo test`; and the rules must agree with `oracle.py`, a
model of "Kitsu's episodes on a tracker's entries" in
`docs/title-resolution.md`, on every generated and recorded case,
unless `allowlist.json` carries the disagreement with a reason.

The rule for a reported tracker mismatch — progress on the wrong
entry, under the wrong number, or a completion at the wrong episode:
it becomes a recorded case in `tests/fixtures/tracker-map/anilist.json`
or `mal.json`, the evidence as the app's readers return it with the
shape the docs answer, in the PR that fixes it. The golden and the
allowlist change only in a PR that changes the rules, with the reason
in the commit body.

## Fixture management

`tests/fixtures/` is the single source of truth for golden data, shared across all test layers.

- Each directory of recorded or synthesized fixtures carries a `MANIFEST.json` recording every fixture's source and SHA-256; `tests/arch/fixture_manifests.sh` checks each manifest against the files beside it. The hand-written inputs (`history/`, `arch/`) carry none.
- Every fixture is small and committed as an ordinary file; git-LFS is not configured.
- There is no refresh target. A fixture is re-captured or regenerated by hand, and its manifest entry is updated in the same change; `tests/fixtures/MANIFEST.md` has the details.

## Property-based testing

Targets:

- **Rust**: `select_quality` invariants, m3u8 rewriter idempotency, URL token roundtrip, history file parse/serialize roundtrip, cache TTL monotonicity.
- **TypeScript**: episode range parser (`"5-7"`, `"-1"`, `"5 6"`), search query sanitizer idempotency, Paraglide message-key existence in every locale.

## Architectural invariants

Cheap grep / AST tests under `tests/arch/`. They fail loudly when boundaries erode.

| Invariant | Tool |
|---|---|
| Frontend imports no Rust types except generated `bindings/*.ts` | custom ESLint rule |
| Every HTTP API handler returns `Result<T, AniError>` | syn-based audit |
| No hardcoded English in `.svelte` files or `.ts` modules under `frontend/src/` (must go through `m.<key>()`) | custom ESLint rule `no-hardcoded-strings`, `i18n-ignore` escape |
| Crate dependency direction (`cache` doesn't depend on `reqwest`, etc.) | `cargo-deny` + `cargo-modules` |

## Mutation testing (deferred)

Trigger condition: CI green for 30 days with total CI duration under 8 minutes.

- **Rust**: `cargo-mutants` nightly, scoped to `proxy/`, `cache/`, `history/`, `scraper/`. Target survival rate < 15%.
- **TS**: `stryker-js` nightly, scoped to `lib/` (DOM mutation noise on components is too high).

## Asserting that something did not happen

A test that waits a fixed time and then asserts that no request went
out, no notice showed or no state changed cannot fail on a slow run:
the subject may not yet have reached the point where it would have
acted, and the negative passes without having checked anything. Wait
instead for a positive witness, something the subject does only once
it is past that point (the next request it sends, a state it reaches),
and assert the negative then. Where the action would begin before a
call returns, such as a click whose handler would send its request
straight away, count it where the request is made rather than where a
handler answers it, and the negative can be read as soon as the call
returns.

A 0 ms timer is a sleep of the same kind wherever the page's requests
pass through MSW: one turn of the event loop is not known to carry
them to a handler. Where every stand-in lives in memory, as in the
unit tests and in acceptance cases built on a fake EventSource alone,
one turn drains every microtask and settles the question.

## Test-discipline expectations for AI agents

Every PR shows the red→green pair in `git log`:

```sh
git log --oneline --grep '^test(red)' | head -20
```

Reconstructing the spec from these commits should be readable. Tests are documentation of intent; commit messages are documentation of motion.
