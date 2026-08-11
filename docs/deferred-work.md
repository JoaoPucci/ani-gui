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

- **Finish moving the script-driven resolution paths to the native
  anidb resolver.** Embedded playback already resolves natively —
  browse search, direct candidate pick, episode-to-master resolution,
  the numbering-offset bridge into `ani-hsts` — and the `-S` index
  handoff is gone from the play path. Still on the script: downloads,
  the external player and Syncplay launch paths, and the
  auto-updater's reasons for existing. Availability never spawns the
  script — it runs the Rust picker — but that picker still queries
  allanime, so its remaining work is the provider migration to the
  native anidb client, not a script removal. The
  botan shim machinery (`anicli/botan_shim.rs`, the PATH provisioning
  in `app.rs`) is dead weight 5.0 never invokes.

  Two separate finish lines, and the gap between them is the thing to
  remember. Native resolution everywhere means every path that
  resolves a stream, including Syncplay and the external player.
  *Deleting* the script is a much wider job — downloads, app startup,
  packaging, the updater and the bats suites all reach for it, and a
  search for the binary name is the only honest way to scope that.

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

- **Windows curl-impersonate transport.** The native anidb client
  needs an impersonating curl to get past the provider's
  TLS-fingerprint front. The Linux packages bundle one; on Windows
  the resolver still falls through to plain curl, which the front
  answers with the interstitial. Waited because the Windows story is
  its own packaging problem — which impersonate build runs under Git
  Bash and how it ships — not a resolver change. Surprising: ani-cli
  5.0 itself has the same gap on Windows.

## Housekeeping

- **Snapshot `$0`: preserve the basename as well as the directory**, if
  a script ever needs it.
