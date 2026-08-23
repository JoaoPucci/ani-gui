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
│   ├── anidb/             # synthesized anidb.app response shapes
│   ├── kitsu/             # JSON:API responses
│   ├── anilist/           # GraphQL responses
│   ├── m3u8/              # master + media playlists, edge cases
│   └── history/           # watch-history samples
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

# Frontend
cd frontend && pnpm test
cd frontend && pnpm test:acceptance

# Architectural invariants (always fast; the workflow-certification
# self-test parses CI config with PyYAML — python3-yaml on Debian/Ubuntu)
bash tests/arch/run-all.sh
```

## Coverage targets

Layer-specific. CI fails on regression below the baseline in `coverage-baseline.json`, not on absolute floors.

The CRAP ceilings (`crap.*` in `coverage-baseline.json`) are a separate, firm gate. A PR that would push a file's CRAP score above `max_le`, or push the count of high-risk files above `high_risk_le`, must refactor (split files, extract helpers, cover more) to bring the code under the ceiling — raising the ceiling to fit new code is not allowed. The tool deliberately leaves `crap.*` untouched on `--update` so the policy is enforced mechanically, not just by reading discipline. A separate `--update-crap` opt-in lets a deliberate cleanup *tighten* the CRAP ceilings (it refuses to write any value looser than the current baseline).

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
| `bash.yml` | PR touches `tests/bash/**`, `tests/arch/**`, or a workflow | yes |
| `rust.yml` | PR touches `backend/**` or `Cargo.lock` | yes |
| `frontend.yml` | PR touches `frontend/**` | yes |
| `arch.yml` | always | yes |
| `mutation.yml` | nightly cron + manual dispatch | no (informational) |

## Fixture management

`tests/fixtures/` is the single source of truth for golden data, shared across all test layers.

- Each subdirectory has a `MANIFEST.json` listing every fixture's source URL, capture date, and SHA-256.
- Fixtures over 1 MB live in git-LFS.
- Refresh via `make fixtures-refresh`, which re-records against live APIs and writes a diff report. The diff is reviewed in the PR.

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
| No hardcoded English in `.svelte` files (must go through `m.<key>()`) | regex test, allowlist for `aria-*`, `data-testid` |
| Crate dependency direction (`cache` doesn't depend on `reqwest`, etc.) | `cargo-deny` + `cargo-modules` |

## Mutation testing (deferred)

Trigger condition: CI green for 30 days with total CI duration under 8 minutes.

- **Rust**: `cargo-mutants` nightly, scoped to `proxy/`, `cache/`, `history/`, `scraper/`. Target survival rate < 15%.
- **TS**: `stryker-js` nightly, scoped to `lib/` (DOM mutation noise on components is too high).

## Test-discipline expectations for AI agents

Every PR shows the red→green pair in `git log`:

```sh
git log --oneline --grep '^test(red)' | head -20
```

Reconstructing the spec from these commits should be readable. Tests are documentation of intent; commit messages are documentation of motion.
