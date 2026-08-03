# Deferred work

Valid work that was found during a change and deliberately not done in
that change. An entry says what the work is and why it waited, plus
anything genuinely surprising about it — enough for someone who was
not in the conversation to pick it back up.

These are reminders, not specifications. An entry does not have to
enumerate the files that will change, state acceptance criteria, or
define what "done" looks like; whoever takes the work scopes it
against the code as it is then, which is the only scoping worth
trusting. So an entry that states something false is a defect, and an
entry that leaves things out is not.

This file is tracked, so an entry survives leaving the checkout it was
written in and a pull request thread can cite it. That is the whole
point: the internal planning directory is git-ignored and this
repository has issues disabled, so neither can hold a record anyone
else is able to read.

Adding an entry is not a way to avoid the work. `AGENTS.md` §14 lists
it third of four options, after doing it here and doing it in its own
pull request.

Remove an entry when the work lands, in the change that lands it.

---

## Port the arch self-tests to bats

`AGENTS.md` §2 settles where these belong: shell with a separate
subject under test needs bats, including under `tests/arch/`, while a
check that inspects the repository and reports what it found stays
standalone under the architectural runner.

`bash_portability_test.sh`, `deferral_record_test.sh` and
`deferral_root_test.sh` are the first kind. They build fixtures, drive
another script through cases and compare results through assertion
helpers written by hand. The checks they drive stay where they are.

Which branch each file currently sits on is deliberately not recorded
here. That is the part of an entry which goes stale first, and it is
the part a reader can establish in a second.

The bats vendor lives under `tests/bash/` behind an installer, so the
move also changes how `run-all.sh` and the workflows invoke the suite.

## Open findings on the deferral checks

Raised in review and not yet fixed. Recorded here so they survive the
thread.

**Intent-to-add detected by porcelain, not by index metadata.**
`record_is_recoverable` rejects `git add -N` entries by matching `" A"`
in `git status --porcelain`. If the file is then deleted from the
working tree the porcelain line changes, and the check stops
recognising it. Reading the intent-to-add bit from index metadata is
the robust form.

There is a deleted-file variant of the same state worth checking at the
same time.

The failure message is wrong for this case too. `why_unrecoverable`
sees that `ls-files` succeeds and reports "tracked as a symlink or
submodule rather than a file", which is neither true nor actionable —
the fix is `git add` on a path that is already, in a sense, added.
Reporting the wrong reason has already been a defect on this branch
once, for symlinks; the same argument applies. Fix it in the same
change as the detection.

**Setext H2 does not end the section.** The body scan recognises
`^ {0,3}## `. A heading written as text followed by a line of `-` is
also an H2, so the section runs past it into the next one.

This belongs to the Markdown-interpretation layer described above, and
the same argument applies: it is the eighth rule of an open set. The
recommendation remains to stop parsing document structure rather than
to add setext handling.

---

# Backlog

Work that is known, wanted and not scheduled. Kept here rather than in
an agent's session state, which nobody else can read and which
disappears when the session does. An item leaves this list by being
done or by being decided against in writing.

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
