# AGENTS.md

Operational contract for any AI agent (Claude Code, Codex, others) working in this repository.

## 1. Project map

`ani-gui` is a fork of [`pystardust/ani-cli`](https://github.com/pystardust/ani-cli) that adds a desktop GUI on top of the existing CLI. The repo holds **two peer artifacts**:

- `ani-cli` (root) — the original 666-line POSIX-shell anime scraper, vendored from upstream and intentionally kept untouched.
- `gui/` — the desktop app. Electron shell (`gui/electron/`) hosts a SvelteKit static SPA (`gui/frontend/`) and launches a Rust sidecar (`gui/backend/`) that resolves streams natively. It touches the script only to keep the bundled copy updated, through `gui/backend/src/anicli/`.

Read first:

- `docs/architecture.md` — components, data flow, why a local desktop app exists at all
- `docs/testing.md` — test pyramid and how to run each layer
- `docs/development.md` — dev environment, build, debug
- `docs/i18n.md` — adding a locale
- `docs/title-resolution.md` — the cross-API bridge (Kitsu ↔ the streaming provider ↔ MAL ↔ aniskip / AniList) and how disambiguation by episode count works
- `docs/proposals/` — future-feature proposals (Cast/multi-viewer, etc.)

## 2. Test discipline (non-negotiable, TDD)

Every change starts red:

1. Write or modify a test first. Commit it failing, with subject prefix `test(red): …`.
2. Make it pass with the minimum code. Commit with prefix `feat(green): …` or `fix(green): …`.
3. Refactor only after green. Commit with prefix `refactor: …` and prove tests still pass.

A PR with a `feat`/`fix` commit lacking a paired `test(red)` predecessor will be rejected. `git log --format='%s' master..<branch-head> | grep '^test(red): '` reconstructs the spec.

The contract binds the fork's own commits. A sync PR imports upstream's history verbatim through a merge (§3), so upstream `feat` commits arrive with no red of ours preceding them — and cannot acquire one without rewriting the vendored history the merge exists to preserve. Whether a commit is upstream's is a provenance question with a mechanical answer: it is reachable from the sync merge's second parent (`git merge-base --is-ancestor <sha> <merge>^2`). The fork's obligation on a sync is the suite rewrite that re-covers upstream's changed behavior, and that is a `test:` commit, not a `test(red):` one — the behavior it pins already ships, so there is no failing state to commit.

**Verify that ordering against the branch, never against a squash preview.** GitHub synthesizes a preview object for every PR: master's head as its sole parent, carrying the entire PR diff. Read as history it always looks like tests and production code landed in one commit, so it manufactures this exact violation for branches that are correctly ordered. Before filing (or accepting) a missing-`test(red)` finding:

```sh
git merge-base --is-ancestor <cited-sha> <branch-head>  # fails → not branch history; you are describing the preview
git log --format='%h %p %s' master..<branch-head>       # the real pairing: each green's parent is its red
git log --format='%s' master..<branch-head> | grep '^test(red): '   # the branch's red SUBJECTS, if any
git merge-base --is-ancestor <test> <fix>               # FAILS → the red does not precede its green
```

Test reachability, not existence. `git cat-file -t` reports only an object's type, so once tooling has fetched the preview it answers `commit` for the preview exactly as it does for a real commit — the two are indistinguishable by that check, and a reviewer who accepts the preview sha reproduces the very false result this procedure exists to prevent. `--is-ancestor` is the check that separates them: it fails for an object that isn't in the branch's history and errors for one that isn't present at all, and both answers mean the same thing here.

A finding is actionable on either path:

- **Red does not precede its green** — the cited objects are reachable from `<branch-head>` and `--is-ancestor <test> <fix>` fails. Assert the invariant directly rather than testing for its negation: `--is-ancestor <fix> <test>` catches only a red committed *after* its green, and says nothing when the two landed on separate branches later merged together. Both are then reachable from the head, neither is an ancestor of the other, and a red-subject search still finds a test — so an unpaired green passes every check. Asking whether the red is an ancestor of the green covers the later-red case and the incomparable case with one question.
- **No red at all** — the subject listing over `master..<branch-head>` shows the branch has no `test(red)` commit covering the behavior the `feat`/`fix` introduced. Filter formatted subjects, not `--grep`: `--grep` matches the whole log message, so `^test(red)` also hits a *body* line and a branch with no red subject can look as though it has one whenever another commit quotes such a line. There is no `<test>` sha to compare in this case, and none is required.

Both outcomes are real. A green-before-red defect has been confirmed this way; so have repeated preview artifacts citing ids absent from the branch. What is never sufficient on its own is a claim about an object that is not reachable from the branch head.

Per layer:

- Bash changes require bats-core coverage (unit, network-mocked, or acceptance as appropriate). This covers the shell *product* — the vendored `ani-cli` script — and any shell with a **subject under test**, including under `tests/arch/`. The question is not how much logic a file contains but whether something else is being exercised: a file that builds fixtures, drives a subject through cases and compares results needs a runner that names the failing case, and hand-rolling that is how you end up with an assertion harness nobody reviews. A file that inspects this repository and reports the invariant it found broken is not in that position however many checks it performs — its output *is* the report, and `tests/arch/run-all.sh` already names which check failed. The existing architectural checks stay standalone on that basis.
- Rust changes require `cargo test`, plus a `proptest` if the function under change is pure.
- Frontend changes require a `vitest` test (component or store) and an acceptance test if a user-visible flow changes.

Architectural invariants in `tests/arch/` are load-bearing. Do not weaken them — extend them.

**How an architectural check may establish its invariant.** Reading
source is allowed — `shellcheck` runs over this repository and is the
right shape of tool. What is not allowed is inferring behaviour from
source with ad-hoc pattern matching. A check may:

- **run its subject** and assert on what it does, or
- **assert a syntactic constraint** — something a `grep` either finds
  or does not, such as a marker at column zero or exactly one mention
  of a declared path per file, or
- **use a real parser**, meaning an existing tool with a grammar, not
  one assembled here.

It may not decide, from regular expressions, what a piece of source
*means*: which `$NAME` is a read from the environment, which
assignment owns it, which text is code at all. Those questions need a
shell parser, and the attempt to build one out of `grep` and `awk`
cost this repository ten review rounds — comments, then quoting, then
heredocs, then heredoc delimiters, then several heredocs per command,
then line continuations, then command-scoped assignment prefixes —
seven of them defects introduced by the fix before. Every one of the
findings was correct, and every fix revealed the next rule of the
grammar.

The distinction is empirical, not aesthetic. On that same branch the
constraint-shaped rules each closed their category permanently and
generated no further findings; the interpretation-shaped ones did not
terminate.

Where the property is behavioural and cheap to exercise, run the
subject. `tests/bash/arch/invocation.bats` establishes that no stray
environment can redirect a check by running each one twice — once
clean, once under a hostile environment — and comparing output and
exit status. It replaced an audit that tried to find the same thing by
reading variable names.

And a check that is deliberately incomplete has to say so. The failure
worth avoiding is not a gap; it is a green run that implies more than
the check knows.

Never modify a test to make production code pass. Modify production code, or change the test in its own `test(red)` commit with a written justification in the body.

The full pyramid (unit → acceptance → e2e → property → architectural invariants → mutation) lives in `docs/testing.md`.

**The CRAP ceilings (`crap.max_le`, `crap.p95_le`, `crap.high_risk_le` in `coverage-baseline.json`) are firm.** A PR that would push a file's CRAP above `max_le`, or push the count of high-risk files above `high_risk_le`, must refactor — split the file, extract helpers, cover more code — rather than raise the ceiling. The historical pattern of bumping the ceiling on every feature was lenient by accident; bringing code under a fixed bar is the actual quality signal.

**The coverage floors (`rust.*`, `frontend.*`, `bash.*`) are also firm — tighten-only.** `node tools/check-coverage-baseline.mjs --update` now refuses to lower a floor below the recorded baseline, even if the current measurement would drop it. A PR that removes coverage must restore it (delete-with-tests, port the assertions to a new layer), not paper over the regression by re-baselining. Use `--update` when tests were deliberately added or scope grew and the floor should genuinely rise; never as a workaround for new code that skipped testing.

**Svelte component logic must be testable.** The M3 design + UX detour shipped several pieces of behaviour inside `.svelte` files (BackButton depth tracking, topbar dropdown state machine, detail-page URL `$effect`s, hero rotation). Mounting Svelte 5 components against SvelteKit's runtime in vitest is brittle, so the rule is: **when you find yourself writing more than a couple of lines of imperative logic inside a `<script>`, extract it into a sibling `.ts` module under `$lib` and unit-test the module.** The component becomes a thin adapter that pulls inputs from the Svelte runtime and hands them to the pure function. `$lib/history/nav-depth.ts` is the canonical example — the layout's `afterNavigate` hook is now four lines of glue around a tested function.

Known test debt (extract + unit-test next time you touch them):

- Topbar live-results dropdown state machine in `+layout.svelte` (debounced search, ↑/↓ navigation, blur-dismiss timing, recent-search persistence).
- Detail-page URL `$effect`s in `routes/anime/[id]/+page.svelte` (`?page=` → `episodesPage`, `?ep=` → `highlightEp` + scrollIntoView, `consumedEp` guard against re-firing).
- Hero rotation timer in `routes/+page.svelte` (3-item cycle, pause on hover/focus, `prefers-reduced-motion` skip).

## 3. CLI script formatting parity (hard rule)

`ani-cli` (the root script) is vendored from upstream `pystardust/ani-cli`. Touching it requires:

- The change must be a behavior change we also intend to upstream — not a stylistic preference.
- Formatting must match upstream's settings byte-for-byte:
  - `shellcheck -s sh -o all -e 2250`
  - `shfmt -i 4 -ci -d`
- Never reformat the script. Never add lint rules to it.

Carried fork patches are the exception, not the rule. Every patch beyond the `__ANI_CLI_LIB__` source-guard line (which lets tests `source` the script as a library) must be marked in-file with an `# ani-gui patch:` comment explaining why it exists. Patches are carried for as long as the bundled script lives — we do not submit them upstream and do not plan around upstream acceptance. The native resolver has since replaced the subprocess, so the script ships only for people who want the terminal flow; the whole carried set retires whenever it stops shipping. Current set beyond the guard: none. The 5.0 sync retired the whole 4.15 set — the greedy name capture and the portable base64 lived in allanime functions 5.0 deleted, upstream absorbed the flatpak directory acceptance, and 5.0's `process_hist_entry` dropped the fallback the history guard existed to guard.

## 4. Layer boundaries

Mechanical rules enforced by `tests/arch/boundaries.sh` and `tests/arch/i18n.sh`:

- `gui/**` may reference `ani-cli` only through `gui/backend/src/anicli/`, which exists solely to locate and auto-update the bundled script. No sourcing, no path references elsewhere, and nothing there resolves a stream.
- The frontend never fetches an upstream URL directly. All stream traffic flows through the local proxy at `http://127.0.0.1:<port>/s/<token>/...`.
- SQLite holds metadata only. Image bytes live on the filesystem under `$XDG_CACHE_HOME/ani-gui/images/`.
- The backend never returns localized strings. It returns stable error keys (`error.search.no_results`); the frontend resolves them via Paraglide.

## 5. Rust conventions (`gui/backend/`)

- Errors: `thiserror`-based `AniError` enum at the library boundary. `anyhow` allowed only inside command bodies.
- Subprocess: `tokio::process::Command` with `kill_on_drop(true)`, `TERM=dumb`, `NO_COLOR=1`.
- HTTP: `axum` for serving, `reqwest` (rustls) for outbound. Two clients: `meta_http` for Kitsu/AniList, `proxy_http` for stream upstream.
- Logging: `tracing` + `tracing-subscriber`. No `println!` in production code.
- Forbidden: `sqlx` (overkill for local SQLite), `actix-web`, `openssl-sys`, `*` version ranges in `Cargo.toml`.

## 6. Frontend conventions (`gui/frontend/`)

- Every user-visible string goes through Paraglide. The `no-hardcoded-strings` ESLint rule enforces this; it allowlists only `aria-*`, `data-testid`, and dev-only strings.
- Use logical CSS properties (`margin-inline-start`, not `margin-left`) so adding RTL locales later is translation-only.
- No DOM-snapshot tests. Assert behavior: rendered text via `i18n.m`, role queries, user events.
- hls.js is used as a singleton inside `Player.svelte`; no `new Hls()` outside that component.

## 7. Design direction guard rails

UI is top priority for this project; defaults to Netflix-style polish. Avoid:

- Generic Tailwind/shadcn dark mode
- Glassmorphism without purpose
- Neon-purple gradients
- AI-styled abstract blob backgrounds
- Auto-rotating carousels (carousels respond to user scroll, not timers)
- Inter-everywhere typography

Embrace:

- Dynamic per-anime theming using AniList's `coverImage.color` for accents on detail/watch pages
- Editorial typography pairing (display face + body face) — pick concretely at the start of M3
- Motion as structure, not decoration: elastic-eased carousels, parallax cards, shared-element page transitions, theater-dim into playback
- Subtle anime motifs: oversized tabular numerals for episode counts, manga-page-inspired dividers used sparingly. No literal sakura or holographic katakana banners.
- Player chrome that auto-hides cleanly (Apple TV+ feel, not VLC)

## 8. `frontend-design` skill usage

When invoking the Anthropic `frontend-design:frontend-design` skill for component generation:

- Always pass design-direction constraints (§7) in the prompt verbatim
- Always run a sub-agent reviewer pass against the output before merging
- Never accept generated code as-is — the skill has produced repetitive AI-styled output before

## 9. UI is top priority

Once a milestone affects the UI surface, treat the work as v1-quality, not as a quick patch. Specifically: M4 ani-skip integration explicitly triggers a UI revisit, since the player overlay changes shape when intro-skip exists.

## 10. PR conventions

Set the assignee on every PR (`gh pr create --assignee @me`). Add a label from the repo's existing set (`bug`, `enhancement`, `documentation`, …) when one obviously fits the change; skip the label otherwise — don't invent new ones without asking.

## 11. System-modifying actions require explicit approval

This rule holds in **all modes**, including auto mode. Pause and surface a request — never silently execute — for any action that:

- Requires `sudo` or any privilege escalation
- Modifies anything under `/etc`, `/usr`, `/opt`, `/var`, system services, or systemd units
- Modifies user-global state outside the repo: `~/.bashrc`, `~/.zshrc`, `~/.profile`, `~/.cargo/`, `~/.rustup/`, `~/.nvm/`, `~/.npm/`, `~/.local/bin/`, `~/.config/` (other than this app's own config), the user's `PATH`, `corepack enable`, `cargo install -g`, etc.
- Installs system packages (`apt`, `dnf`, `pacman`, `brew`, `pkg`, `snap`, `flatpak install`)
- Modifies firewall rules, network config, environment variables persisted to shell rc files
- Affects any file outside the repo working directory tree

Project-local writes inside the repo are always fine (the test toolchain installer at `tests/bash/helpers/install-bats.sh` writes only to `tests/bash/.bats-vendor/`, which is gitignored — that's project-local and proceeds).

When pausing for approval, explain what the action does, why it's needed, and what the alternative is if the user declines. Examples of safe alternatives: ship a Docker dev image rather than asking the user to install system packages; vendor a tool into the repo rather than `cargo install -g`; document the requirement in `docs/development.md` so the user installs it themselves.

## 12. Git hygiene

- **Stage files individually, by full path.** Never use `git add .`, `git add -A`, `git add -u`, or directory-level adds like `git add docs/`. Each file goes into the index by name. This forces an intentional review of every file in every commit and prevents accidental inclusion of secrets, scratch files, or unrelated edits.
- **Commit subjects use the conventional prefix matching the change kind**: `test(red): …`, `feat(green): …`, `fix(green): …`, `refactor: …`, `chore: …`, `docs: …`, `chore(deps): …`, `chore(ci): …`. Anything that introduces a `feat` or `fix` must have a paired predecessor `test(red): …` commit (see §2).
- **No `git push --force` to `master`.** If a force-push is genuinely needed (e.g. accidentally committed credentials), pause and confirm with the user before doing it.
- **Never `--no-verify` past hooks** unless the user explicitly directs it. If a pre-commit hook fails, fix the underlying issue.

## 13. Release & versioning

- **No post-tag pre-bump.** Do not bump the version after cutting a release. `master` stays at the **last shipped version**; the bump happens **inside the release PR** itself, so `master` never runs ahead of what's published.
- **The four version files move in lockstep** in that release PR: `gui/backend/Cargo.toml`, `gui/backend/Cargo.lock` (`cargo update -p ani-gui`), `gui/frontend/package.json`, `gui/electron/package.json`.
- **Hotfixes branch off `master`.** Because `master == last shipped`, a patch branches straight off it — apply the fix, bump the patch version in the same PR, merge, tag from `master`. No release branch, no version gap to bridge. (Lesson from the v0.9.1 Wayland-crash hotfix: a prior post-tag pre-bump had stranded `master` ahead of the released line, forcing an awkward version downgrade-merge that regressed `master`.)
- **Milestones are decoupled from the version file.** After a tag, create the next-minor GitHub milestone for tracking and assign new PRs to it explicitly — do **not** infer the milestone from the in-tree version.
- **Releases publish as pre-releases** (`gh release create --prerelease`); pre-1.0, none are promoted to "Latest".

## 14. Scope is negotiable, delivery is not

Never drop a piece of work on the grounds that it is too large. "Too
much for this change" is a statement about where the work goes, not
about whether it happens.

When a reviewer raises something valid, or you find something valid
mid-task, exactly one of these is an acceptable outcome:

- **Do it here.** The default. Estimate the work before declining it —
  an estimate made in order to justify not doing something tends to
  come out high.
- **Do it in its own PR.** Open the follow-up PR, land it, then update
  the original. Say on the thread which PR carries it.
- **Add an entry to the deferred-work log**, and link it wherever you
  deferred it. An unwritten deferral is a dropped one, and a deferral
  written only where you happen to be standing is the same thing: the
  internal planning directory is ignored by git, so an entry there
  leaves with your checkout and a thread citing it points at a file
  nobody else has. Issues are disabled on this repository, so they are
  not an option either. Keep the internal queue if it helps you — the
  durable record, the one you cite, is the tracked log.

  Write enough to pick the work back up later and no more: what it is,
  why it waited, and anything genuinely surprising about it. The log is
  a set of reminders, not a specification. It does not want a file
  list, acceptance criteria, or a plan — whoever takes the work scopes
  it against the code at the time, which is the only scoping worth
  trusting.

  Reviewing an entry follows from that. An entry that says something
  false is a defect: it would send a reader to rebuild what already
  works, or let them believe the job is done when it is not. An entry
  that leaves things out is not a defect, and asking for the omission
  to be added is how a reminder turns into a specification. If a `grep`
  would surface it, leave it out.

  Any path declared below is checked for being readable from a fresh
  clone, by `tests/arch/deferral_record.sh`.

<!-- record-path: docs/deferred-work.md -->
- **Ask.** If the tradeoff is genuinely the maintainer's call, put the
  options to them. Silence is not a way to ask.

What is never acceptable is the fifth outcome: replying that the fix
is out of scope or too costly and leaving nothing behind. That reads
as a considered engineering judgement while being, in effect, a
refusal — and the work is lost, because nothing records it.

The same rule applies to your own estimates. Before invoking cost,
check the cheap path actually is closed: existing conventions in the
package, a helper already extracted, a test glob that already covers
the directory. More than once the "large refactor" turned out to be
one new file next to two just like it.

## 15. Pointers

- `docs/architecture.md` — public architecture
- `docs/testing.md` — test pyramid, fixture management, coverage targets
- `docs/development.md` — dev setup
- `docs/i18n.md` — locale-addition guide
- `docs/proposals/cast-multiviewer.md` — Cast/multi-viewer future-feature proposal
