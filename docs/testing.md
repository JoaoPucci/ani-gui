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
   │              End-to-end              │  Playwright + Electron,
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
└── (e2e lives in electron/e2e/, see below)
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

# Architectural invariants (always fast; ci_caches.sh, and the bats
# workflow-certification self-test, parse CI config with PyYAML —
# python3-yaml on Debian/Ubuntu — and fail without it)
bash tests/arch/run-all.sh

# The season picker against the model of its docs (tests/picker-oracle/README.md)
python3 -I -m unittest discover -s tests/picker-oracle -p 'test_*.py'
```

The integration tests under `backend/tests/` that start the real
backend binary keep it off your own profile on every platform:
`backend/tests/common` sets `ANI_GUI_TEST_PROFILE_ROOT` to each test's
temporary folder, and the backend then keeps its config, cache
(`metadata.sqlite`, images, the flags cache), state (history, offsets),
logs and default download folder under that one folder, ahead of the
XDG variables, `HOME` and the platform's folder APIs. Without it,
Windows would hand the binary the machine's own profile (`ani-gui-dev`
for a debug build, `ani-gui` for `--release`): it resolves those
folders through the known-folder API, which no other variable
redirects. On Linux and macOS `HOME` and the XDG variables would
redirect them, but under a layout that differs between the two; under
the root it is one layout everywhere. The variable is for the
backend's test harness only: nothing in the app sets it, and
`tests/arch/test_profile_root.sh` keeps its name to the module that
reads it, the backend's tests and these docs.
`backend/tests/profile_root.rs` starts a backend with it and with every
home and folder variable pointed at a sentinel folder, has it boot,
write its settings and its history, and checks that everything it
reports and writes is under the root and that the sentinel stays
empty. The sentinel proves something on Linux and macOS only, where
those variables are where a stray path would come from.

## End-to-end (Playwright + Electron)

The suite under `electron/e2e/` drives the packaged app
(`electron/dist/linux-unpacked/`). Every spec launches it through
`electron/e2e/launch.ts`, which mutes it and refuses to open it on a
display someone may be looking at (`electron/lib/e2e-display.cjs`).
A spec that has to own what the app's stdout and stderr lead to — a
pipe it closes, a terminal it hangs up (`terminal-closed.spec.ts`) —
launches it with `spawnPackagedApp` from the same helper, as a plain
child process rather than through Playwright's driver, with the same
switches, display check, environment and policy guard (over the app's
output alone, since Playwright is not attached to its pages).

The same helper keeps the backend off the network. A spec can set its
stubs only once the app is up, by which time the first page has
already made its requests. Every launch therefore points the backend's
proxy settings at a local proxy that refuses every connection and
records it (`outboundAttempts`). Loopback is exempt, so a spec's own
local servers still answer. `smoke.spec.ts` checks that a request only
GitHub could answer ends up at that proxy instead. A spec whose page
should run on stubs alone does four things: it finds the app's window
with `appWindow` before setting any route, sets `refuseUnstubbed`
(which takes that window) before its stubs, mounts the page again with
`remountFirstPage`, and asserts that no request was left unanswered.
`smoke.spec.ts` and `keyring-unresponsive.spec.ts` do. The window comes
first because when every route is set while the window is still being
attached, none of them reaches it.

A stub answers through `answer` in `electron/e2e/stubs.ts`, never in
place inside Playwright. `answer` holds what the stub says at a local
stub server (`electron/lib/e2e-stub-server.cjs`) and sends the request
on to it, so the answer comes back over real HTTP through Chromium's
network stack, as the backend's does. The stub server answers an API
request as the backend would: the gate in front (the launch's secret as
the header, on every route; an empty 403 otherwise) and the
backend's CORS rules on the answer. A preflight never reaches it:
Playwright answers preflights itself, for any origin, so the CORS a
spec meets is the answer's own. An answer to the request made in place
loses its status and headers once the main process has a webRequest
listener on the window's session, as it has to add the secret header
(`electron/lib/secret-header.cjs`): every stubbed page then reads its
data as unreachable. `tests/arch/e2e_stub_server.sh` keeps
Playwright's in-place answer out of `electron/`.

Every spec takes `test` and `expect` from `electron/e2e/policy-guard.ts`,
not from `@playwright/test`, and `tests/arch/e2e_policy_guard.sh`
checks that each spec imports from it. The launch helper guards every
app it starts: it reads the app's output for content security policy
violations, the first page's included. The guard's `test` then fails
a test on any violation it did not declare with `expectViolation`.

A spec that plays a stream starts it as a viewer does, by clicking the
home page's Continue card (`electron/e2e/real-play.ts`), and the play
runs through the backend: its play route, its session and its stream
proxy. The one step it cannot take is the provider walk, since the
backend is kept off the network. Instead the spec's profile turns on
replaying resolved plays (`cache_resolutions`), and when the renderer's
play request arrives, the spec writes the row that play reads into the
backend's resolution cache, naming a stream the spec serves, before
letting the request through (`electron/lib/e2e-replay.cjs`).
`lanes.spec.ts`, whose subject is the connections each lane holds,
writes the row itself and asks the play route directly, before it
holds the resolve lane that route travels on. The row
is written with Node's built-in `node:sqlite` module, so the suite and
the Electron helper tests (`electron/lib/*.test.cjs`) need Node 22.13 or
later (CI runs them on Node 22). The row's key prefix,
insert statement, lifetime and fields are kept in
`electron/e2e/fixtures/replay-row.json`, and the backend's tests check
that file against what the cache reads
(`play_resolution_cache_e2e_row_test.rs`), so a change to the cache's
schema fails there, by name, and not as a play that times out. A
hand-off to an external player or Syncplay is never passed to the
backend, because with replays on it would launch the configured
program. The stub answers as the backend does when that program will
not start.

Each launch runs on a scratch profile the spec makes under the
system's temporary directory. The spec passes that directory to
`launchPackagedApp` as `profile`. The helper removes that path, and
nothing else, once the app has exited. Because the backend can go on
writing there briefly while it stops, the helper removes the path
again once the backend has had time to stop, or as the test worker
exits if that comes first. A launch that never starts has its profile
removed at once. `smoke.spec.ts` checks that the profile is gone after
the app quits. Two specs manage their profile themselves, removing it
in their own teardown and, if the launch fails, at once: the keyring
spec, whose profile also holds its session bus, and the splash-stuck
spec, whose restarted app outlives the launch. `terminal-closed.spec.ts`
launches through `spawnPackagedApp`, which takes no profile: the spec
looks at the profile after the app has exited (for the single-instance
lock) and only then hands it to the helper's `removeAfterExit`. A worker
that is killed before its app exits leaves the profile behind.

The Node tests that make temporary directories (`electron/lib/`,
`tests/tools/`, `tests/complexity/`) remove them in their own
teardown.

Locally, the suite runs on Wayland, inside a private headless GNOME
Shell session that `tools/headless-wayland.sh` starts and tears down:

```sh
cd electron
pnpm run package      # when main, preload or the backend changed
../tools/headless-wayland.sh pnpm run e2e:fast
```

The session has its own runtime directory, D-Bus session, HOME and XDG
directories, and no `DISPLAY`, so the app's windows open on a
compositor nobody can see, and a setting changed on the desktop does
not reach it. It needs `gnome-shell`, `dbus-run-session`,
`dbus-update-activation-environment`, `setsid`, and systemd 254 or later
(`systemd-run`, `systemctl`) with a user service manager running: the
whole session runs in a transient scope of its own (the tool prints its
name, `ani-gui-wl-<suffix>.scope`), and what is still
running in it at the end — a service that daemonized, a process that
made a session of its own — is stopped by stopping that one scope. The command
runs without a terminal, so the tool is for the suite, not for an
interactive shell. The compositor keeps the desktop's system bus, which
it cannot start without; its settings forbid it to lock, since a lock
would reach the desktop's login session through that bus. Outside CI, a launch without the session — from a
desktop shell, or under `xvfb-run`, where the packaged launcher still
picked the desktop's Wayland compositor — throws before any window
opens, saying how to run the suite.

A spec stops the app it launched through `stopApp` in
`electron/e2e/launch.ts`, and nothing else but the exceptions
below. Playwright starts the app
in a process group of its own; `electron/lib/own-group.cjs` sends
SIGTERM to the app's main process alone, then SIGKILL to its group if
the app is still there, and only while the group is provably the app's:
its main process not yet reaped, leading the group, running the binary
it was launched from. SIGTERM to the whole group would kill Chromium's
helpers in the same instant as the main process, which then aborts as
it quits, missing its GPU process (a SIGTRAP the host's crash reporter
is called for). An app that ends by a signal the stop did not send
crashed as it quit, or, before the stop, crashed or was killed from
outside it, and `stopApp` throws, failing the spec; the cold-launch retry's cleanup (`stopDeadApp`) discards that
error, as it does any other from stopping the launch it gives up on. What the app leaves in its group as it exits is killed in the
turn its exit is reported; after that nothing is signalled, since an
emptied group's id can be handed to someone else. The backend runs
outside that group and stops itself when the app's end of its stdin
closes. No test or tool picks a process out of the process table — by
name, path, command line, environment, or parent: a test that walked
from a relaunched app to its parent once signalled the user's service
manager and ended the desktop session. `tests/arch/own_processes.sh`
catches the usual spellings of such a selection (the selecting tools,
the parent-pid field's name, listings of `/proc`); its header lists
what it cannot see.

CI runs the same suite on X11, under Xvfb (`xvfb-run`): GitHub's
runners have no Wayland. Wayland behaviour is covered by local runs
only. The launch helper accepts Xvfb only on a GitHub Actions runner
(`GITHUB_ACTIONS=true`), and refuses any Wayland session there.
`smoke.spec.ts` checks that the app landed on Wayland in the headless
session and on X11 in CI.

`terminal-closed.spec.ts` stops its apps itself, under the same
rules. The app it starts on a pipe is its own child, leading a group of
its own: it is stopped with `own-group.cjs`'s `stopOwnGroup` directly,
since it is not a Playwright launch, and `spawnPackagedApp` kills what
it leaves in its group in the turn its exit is reported, or the whole
group if the test worker exits while it is still unreaped. On a
terminal, the launcher it starts is the child of the terminal fixture
(`electron/e2e/fixtures/terminal/on-a-terminal.py`, which needs
`python3`): the fixture signals the launcher's group only before it
has reaped the launcher, kills what is left in the group once the
launcher has exited and before reaping it, and kills the whole group
if the spec's end of its stdin closes first, as it does when the test
worker exits. The app itself is the launcher's child, in a session of
its own outside that group (`electron/lib/linux-launcher.cjs`): a
killed launcher leaves it a SIGTERM, and an app that does not quit on
it outlives the test.

`electron/lib/linux-launcher.test.cjs` runs the launcher with a
stand-in for the app's binary (`electron/lib/fixtures/app-stub.py`),
on that terminal fixture too, so it also needs `python3`; it is
skipped off Linux. Every process it signals is the launcher it
started, by pid or group, or the stand-in, by the pid the stand-in
recorded.

One process is not launched by the helper: the app the splash's Restart
starts again (`splash-stuck.spec.ts`). It is outside the test's process
tree and group, reparented to the session's subreaper when its parent
exits, so `stopApp` cannot reach it and nothing may walk to it. The
spec takes its pid from what the app writes into the spec's own
temporary profile (the single-instance lock, a symlink to
`<host>-<pid>`), and signals that one pid only after `/proc/<pid>/exe`
shows it running the spec's own copy of the binary.
`splash-stuck.spec.ts` runs a hard-linked copy of
`dist/linux-unpacked/` whose `resources/frontend/build` holds a fixture
page (`electron/e2e/fixtures/first-page/`), so the late first page is
real and nothing in the app reads a test switch.

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
| `bash.yml` | PR touches `tests/bash/**`, `tests/arch/**`, a script the suites execute, a workflow, or a composite action under `.github/actions/` | yes |
| `rust.yml` | PR touches `backend/**`, `tests/fixtures/**`, `rust-toolchain.toml`, `tests/picker-oracle/fixtures-golden.json`, `tests/picker-oracle/cases/**`, `tests/tracker-map-oracle/fixtures-golden.json`, `tools/hostile-tmpdir.sh`, `tools/rust-lcov.sh`, `.github/actions/apt-install/` or the workflow — decided by `tools/rust-workflow-filter.sh`, which `tests/bash/arch/rust_workflow_filter.bats` runs | yes |
| `rust.yml`, job `picker oracle agreement` | PR touches `backend/**`, `tests/fixtures/hianime/picker/**`, `tests/picker-oracle/**`, `docs/title-resolution.md`, `tools/alongside.sh` or the workflow — the same script | yes |
| `rust.yml`, job `picker oracle fuzz` | the same changes as `picker oracle agreement`; draws 20000 pools from the run number as its seed and prints the seed | yes |
| `rust.yml`, job `episode map oracle agreement` | PR touches `backend/**`, `tests/fixtures/tracker-map/**`, `tests/tracker-map-oracle/**`, `docs/title-resolution.md` or the workflow — the same script | yes |
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

## The wire contract

The renderer's types for what crosses the HTTP seam (`frontend/src/lib/api.ts`,
`account/types.ts` and the modules beside them) are written by hand; no
generator makes them from the backend's structs. Two tests keep the sides
saying the same thing:

- `backend/src/api/wire_contract_test.rs` builds `backend/wire-contract.json`
  from the backend's own types — a full and a minimal sample of each
  response type the renderer reads, the fields each request body reads and
  requires, every value of each enum the renderer names (and the names the
  launch announces each connection lane under), the error envelope
  of every variant, and the values the renderer keeps a copy of (the Watch
  Later bridge cap, the names of the secret and refill headers, and the
  proxy's media URL shape; the Electron main process's own copy of the
  secret header's name is not in the contract, and
  `electron/lib/secret-header.test.cjs` holds it to the backend's source
  instead) — and
  `cargo test` fails until the file says the same. A field added to a
  published type, or a variant to a published enum, does not compile until
  the samples carry it.
- `frontend/src/lib/wire-contract.test.ts` holds each renderer type to that
  file through a spec the compiler derives from the type, and names each
  field where they disagree: missing, extra, optional when always sent (or
  the reverse), or of another JSON kind.

A change to what the backend sends fails the first; replace the file with
the contents its message prints, and the second then names what the
renderer's types must change. Both test files' headers list what they do
not hold, among them answers that are not a struct, map values, request
value kinds, and URLs built as text.

The tests' fake backend is held to the same types, so a fake does not keep a
test green on an answer no backend sends. An MSW handler names the route's
type at the answer (`HttpResponse.json<KitsuAnimeRef[]>([...])`), which the
type check reads; stream event data goes through `wire<T>`; the unit
tests' `fetch` stubs (`mockFetchOnce<T>`) and the stand-ins for API
functions and injected dependencies (`vi.fn<typeof f>()`) take the type of
what they replace. `frontend/tests/answers.ts` carries the MSW side:
`errorAnswer` for an error and its envelope, and `malformedAnswer` for an
answer that is wrong on purpose, so it reads as deliberate. The builders
that fill the fields a test does not care about with values the backend
sends (`kitsuRefAnswer`, `sessionAnswer`, `configAnswer` and the rest), with
`wire` and `malformedAs`, live in `frontend/src/lib/testing/wire-answers.ts`,
where the unit tests can reach them (they import nothing from outside
`src/`; `tests/arch/dep_direction.sh` holds that), and `tests/answers.ts`
re-exports them. ESLint refuses an `HttpResponse.json` with no type, or one
that types nothing, and app code importing through a path segment named
`testing` (but not a path built at run time, or a glob that reaches it
through a wildcard);
`tests/lint/typed-answers.test.ts` lists what it does not see, such as a
`vi.fn()` left untyped, which only review catches. When a renderer type
changes, the type check names every typed fake that no longer matches it.

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
| No hardcoded English in `.svelte` files or `.ts` modules under `frontend/src/` (must go through `m.<key>()`) | custom ESLint rule `no-hardcoded-strings`, `i18n-ignore` escape |
| Crate dependency direction (`cache` doesn't depend on `reqwest`, etc.) | `tests/arch/dep_direction.sh` (grep) |
| Only the backend's test harness names the test-only profile root (`ANI_GUI_TEST_PROFILE_ROOT`) | `tests/arch/test_profile_root.sh` (grep) |

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
returns. Where the subject waits on a timer of its own before it
acts, as the topbar's live search waits for the typing to pause, the
witness is that timer's end: `frontend/tests/acceptance/scheduled-timers.ts`
follows the timers an action arms until each has fired or been
cleared, and what the subject asks on them is recorded as it asks.

A 0 ms timer is a sleep of the same kind wherever the page's requests
pass through MSW: one turn of the event loop is not known to carry
them to a handler. Where every stand-in lives in memory, as in the
unit tests and in acceptance cases built on a fake EventSource alone,
one turn drains every microtask and settles the question.

Motion in the acceptance tier has the same shape in both directions.
Under the tier's `element.animate()` stub a Svelte transition starts
its timed animation a task after the update that changed the DOM, an
element an outro holds stays in the DOM until that animation ends, and
a scripted scroll can wait a frame for layout. So the DOM change is no
witness for the motion it causes: read too early, "it moved" fails and
"nothing moved" passes without having looked. A motion scenario waits
for the recorder in `frontend/tests/acceptance/motion.ts` to report
`settled()` (no animation still running; the stub's
`animationsInFlight()` is the same count), or for the scroll to
appear in `scrolls`, and asserts then.

## Test-discipline expectations for AI agents

Every PR shows the red→green pair in `git log`:

```sh
git log --oneline --grep '^test(red)' | head -20
```

Reconstructing the spec from these commits should be readable. Tests are documentation of intent; commit messages are documentation of motion.
