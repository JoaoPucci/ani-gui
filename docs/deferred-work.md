

---

# Backlog

Work that is known, wanted and not scheduled. Kept here rather than in
an agent's session state, which nobody else can read and which
disappears when the session does. An item leaves this list by being
done or by being decided against in writing.

**These entries were carried across from session state and have proven
unreliable about their own status.** Review of this change corrected
more than a dozen, in three kinds: most described work that had
already shipped; two asserted a problem that never existed in the form
stated; one was accurate but named so ambiguously that a careful
reader took it for finished work. Every one was caught here rather
than by the person who wrote the list.

No exact figure is given deliberately. An earlier draft said "nine",
went stale within the same review as further rows were pruned, and had
to be corrected — a fixed count inside a warning about staleness
decays exactly like the rows it describes. Confirm an item against the code before starting it,
and delete it here when you find it done — the list earns trust by
being pruned, not by being long.

A correction rate that high is not a list with a few stale rows; it is
a list whose status field means nothing yet. A full audit against the
code is worth doing before anyone plans from it. Until that happens,
treat every entry as a lead rather than a fact.

## Resolver and provider

- **Replace the bundled `ani-cli` with a native Rust resolver.** Search
  and episode-count disambiguation are already native, and so is show
  metadata: `fetch_show` issues the same `availableEpisodesDetail`
  query as the script's `episodes_list` and exposes the per-mode
  episode tags through `ShowMetadata`. The native boundary is not
  candidate selection — it stops before stream-source resolution.
  Everything past that point is script-only.

  There are two finish lines and they need different treatment.

  **Native playback** is bounded and derived from the code:

  1. **The episode-source request.** `get_episode_url` sends the
     authenticated persisted GraphQL query for `(showId,
     translationType, episodeString)` and receives the encrypted
     `tobeparsed` response.
  2. **Key derivation and source decryption**, which is what the
     provider's change broke.
  3. **Turning a decrypted `sourceUrl` into a playable stream** —
     `generate_link` / `get_links`: provider embed requests,
     master-playlist expansion, quality selection, referer selection.

  **Deleting the script is deliberately not enumerated here.** Seven
  attempts to list it were each corrected by someone reading the
  repository: downloads, the boot path, the packaging entry, the test
  suites, and once in the opposite direction when it claimed show
  metadata was still script-only. An eighth list would carry the same
  authority as the seven wrong ones.

  Derive it instead. Search the repository for the binary name and
  handle every hit — at the time of writing that reaches
  `commands/download.rs` (`spawn_download` runs `ani-cli -d`),
  `AppState::build` (`locate_ani_cli` and `resolve_anicli_path`
  propagate, so the app will not start without it), the
  `electron/package.json` staging entries, the `-U` updater and the
  diagnostics and settings surfaces reporting on it, twenty `.bats`
  files under `tests/bash/` whose `helpers/loader.bash` sources
  `$REPO_ROOT/ani-cli`, `tests/arch/bash_portability.sh`, and the Bash
  CI workflow that triggers on the path. Those are examples of what the
  search finds, not the answer.

  Note that native playback does not shorten this list. Every consumer
  above survives a fully native player, so the two finish lines are
  sequential and independent, and reaching the first is not progress
  toward the second.

  Retires the carried patch set in `AGENTS.md` §3 by deleting the
  script, but only once all of that exists.
- **The provider's crypto flow changed and playback is broken.**
  Upstream has two competing unmerged fixes. The smaller one
  identifies the real inputs — a lane parameter, build id, locally
  generated mask and epoch feeding a bootstrap header — which is a
  pure function and therefore testable against captured fixtures
  without a subprocess. That makes it the smallest native slice worth
  taking first.
- **Validate the botan wrapper on a packaged Windows build** under Git
  Bash.
- **JSON-escape `$1` in `search_anime`** — a carried fork patch that
  was never written.

## Correctness in the app


## Testing and CI

- **The CRAP ratchet disagrees between CI and local** — 26 against 25 —
  and three files sit at 29.7–30.0, right on the high-risk boundary.
- **The pre-commit hook and strict TDD are in tension — for frontend
  commits.** `frontend-test` is the only command in the hook that runs
  tests; Rust gets `cargo fmt` and shell gets `shellcheck`, so a red
  commit touching those was never blocked. A frontend `test(red):`
  commit fails by construction and the hook rejects it.

  This has been worked around with `--no-verify`, which disables every
  other check the hook performs — including the ones that would have
  caught a failing suite before it was pushed. The workaround is the
  bug, not the strictness.

  **The fix has a constraint worth knowing:** pre-commit cannot see the
  commit message. Git writes `COMMIT_EDITMSG` after pre-commit runs,
  even for `git commit -m` — verified with a probe hook. So the test
  gate has to move to `commit-msg`, which receives the message file,
  and skip only when the subject begins `test(red):`. Everything that
  does not depend on intent stays in pre-commit.

  **Done looks like:** a red frontend commit passes without
  `--no-verify`, every other commit still runs the suite, and a failing
  `commit-msg` hook is confirmed to abort the commit under lefthook.

- **Port the arch self-tests to bats.** `AGENTS.md` §2 now says shell
  with a subject under test belongs in bats. Three files qualify:
  `tests/arch/deferral_record_test.sh`, `deferral_root_test.sh` and
  `bash_portability_test.sh` — each builds fixtures, drives another
  script and asserts on results through hand-written helpers.

  All three arrive on unmerged branches, so none is on `master` yet and
  this is sequenced after they land. The subject the third one drives,
  `tests/arch/bash_portability.sh`, *is* on `master` — it is the check
  itself, which stays standalone under the rule above and is not part
  of this port. The bats vendor lives under
  `tests/bash/` behind an installer, so the port also changes how the
  arch suite is invoked and how `arch.yml` runs it.

  **Done looks like:** the three run under bats, `run-all.sh` and
  `arch.yml` invoke them accordingly, and the rule in §2 is satisfied
  rather than acknowledged.

## Interface

- **Localised content fetch** — synopsis and episode titles.
- **Franchise and season grouping** across surfaces.
- **Play-page keep-alive → normal reload with a persisted position.**
- **Search has no sort *direction* control.** Sorting by relevance,
  title, year and rating ships, as do subtype filter chips; only
  ascending/descending is absent. Name any further filters wanted
  before starting, rather than reading this as filters being missing.
- **Update notifier is not resilient to GitHub rate limits.**
- **Adopt the `documentPictureInPicture` browser API** for the player's
  pop-out window. Not a request to write documentation — "Document
  Picture-in-Picture" is the W3C API's name, which reads as an
  imperative and has already been misread once.

  Electron exposes the API but omits the window-creation glue, so it
  cannot work today: electron/electron#39633, open since 2023. Do not
  re-attempt until that lands. PiP as it exists now — the singleton
  video that survives navigation — ships and is described in
  `README.md` and `docs/architecture.md`.
- **Illustrated brand assets** — post-1.0.

## Housekeeping

- **Snapshot `$0`: preserve the basename as well as the directory**, if
  a script ever needs it.
