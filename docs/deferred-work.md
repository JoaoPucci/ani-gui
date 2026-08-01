

---

# Backlog

Work that is known, wanted and not scheduled. Kept here rather than in
an agent's session state, which nobody else can read and which
disappears when the session does. An item leaves this list by being
done or by being decided against in writing.

**These are reminders, not specifications.** An entry has to be true
and has to be enough to pick the work back up; it does not have to
enumerate every file that will change, list acceptance criteria, or
survive as a plan. Whoever does the work scopes it against the code at
the time, which is the only scoping that can be trusted anyway.

So when reviewing this file: an entry that states something false is a
defect, and an entry that leaves things out is not. If reading it
would send someone to rebuild what already works, or let them think
the job is finished when it is not, say so. If it merely omits a
detail a `grep` would surface, that is the intended level of detail.

**Treat every entry as a lead, not a fact.** These were carried across
from session state and reviewing them turned up more than a dozen
errors — mostly work that had already shipped, a couple of problems
that never existed as described. Check an item against the code before
starting it, and delete it when you find it done.

## Resolver and provider

- **Replace the bundled `ani-cli` with a native Rust resolver.** Search,
  episode-count disambiguation and show metadata are already native, and
  so is everything downstream of a resolved URL — `commands/play.rs` and
  `proxy/m3u8.rs` handle sessions, manifests and segment signing. What
  is still script-only is the stream-source resolution in between:
  the authenticated episode-source request, the key derivation and
  decryption the provider's change broke, and `generate_link` /
  `get_links` turning a `sourceUrl` into a playable stream.

  Two separate finish lines, and the gap between them is the thing to
  remember. Native playback means every path that resolves a stream,
  including Syncplay and the external player. *Deleting* the script is
  a much wider job — downloads, app startup, packaging, the updater and
  the bats suites all reach for it, and a search for the binary name is
  the only honest way to scope that. Retires the carried patch set in
  `AGENTS.md` §3, but only at the end of the second job, not the first.

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
  commits.** `frontend-test` is the only hook command that runs tests,
  so a frontend `test(red):` commit fails by construction and is
  rejected. It has been worked around with `--no-verify`, which
  disables every other check too; the workaround is the bug.

  The non-obvious part: pre-commit cannot see the commit message. Git
  writes `COMMIT_EDITMSG` after pre-commit runs, even for `git commit
  -m` — verified with a probe hook. So the gate has to move to
  `commit-msg` and skip only for a `test(red):` subject.

- **Port the arch self-tests to bats.** `AGENTS.md` §2 says shell with a
  subject under test belongs in bats. Three qualify —
  `tests/arch/bash_portability_test.sh` (on `master`) plus
  `deferral_record_test.sh` and `deferral_root_test.sh` (still on a
  branch) — each builds fixtures, drives another script and asserts
  through hand-written helpers. The checks they drive stay standalone.

  The bats vendor lives under `tests/bash/` behind an installer, so the
  port also changes how `run-all.sh` and `arch.yml` invoke the suite.

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
  Picture-in-Picture" is the W3C API's name.

  Electron exposes the API but omits the window-creation glue, so it
  cannot work today: electron/electron#39633, open since 2023. Do not
  re-attempt until that lands. PiP as it exists now — the singleton
  video that survives navigation — ships and is described in
  `README.md` and `docs/architecture.md`.
- **Illustrated brand assets** — post-1.0.

## Housekeeping

- **Snapshot `$0`: preserve the basename as well as the directory**, if
  a script ever needs it.
