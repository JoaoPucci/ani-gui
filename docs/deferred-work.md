

---

# Backlog

Work that is known, wanted and not scheduled. Kept here rather than in
an agent's session state, which nobody else can read and which
disappears when the session does. An item leaves this list by being
done or by being decided against in writing.

**These entries were carried across from session state and have proven
unreliable about their own status.** Nine were already shipped, every
one caught in review of this change rather than by the person who
wrote the list. Confirm an item against the code before starting it,
and delete it here when you find it done — the list earns trust by
being pruned, not by being long.

Nine out of roughly thirty is not a list with a few stale rows; it is
a list whose status field means nothing yet. A full audit against the
code is worth doing before anyone plans from it. Until that happens,
treat every entry as a lead rather than a fact.

## Resolver and provider

- **Replace the bundled `ani-cli` with a native Rust resolver.** Search
  and episode-count disambiguation are already native —
  `scraper/allanime.rs` stops there. Two things remain only in the
  script, and both are needed before playback works natively:

  1. **Key derivation and source decryption**, which is what the
     provider's change just broke.
  2. **Turning a decrypted `sourceUrl` into a playable stream** —
     `generate_link` / `get_links`: provider embed requests,
     master-playlist expansion, quality selection, referer selection.

  Doing only the first produces correct ciphertext handling and still
  no URL to play. Retires the carried patch set in `AGENTS.md` §3 by
  deleting the script, but only once both exist.
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

  Sequenced after the branches carrying those files land, since two of
  the three do not exist on `master` yet. The bats vendor lives under
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
