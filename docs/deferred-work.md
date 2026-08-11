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

Write an entry about the code, not about the work in flight. A commit
sha, a pull-request number, a branch name, a commit count, or a
sentence like "the PR for this is open" is accurate for about a week
and misleading forever after — and a rotted entry is worse than no
entry, because it sends the next reader to rebuild finished work or to
reason from a state that no longer exists. Name the behaviour that is
wrong and where it lives; that stays checkable. References that do not
rot — an upstream issue, a specification — are fine.

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

- **Bundle the curl-impersonate transport in the Windows package.**
  The native anidb client needs an impersonating curl to get past the
  provider's TLS-fingerprint front; without one the resolver falls
  through to plain curl, which the front answers with the
  interstitial. The Linux packages stage one; Windows is untouched,
  and the open question there is which impersonate build runs under
  Git Bash and how it ships. Waited because packaging is its own
  problem, not a resolver change. Surprising: ani-cli 5.0 itself has
  the same gap on Windows.

- **Nothing enforces the red-before-green pairing.** `AGENTS.md` §2
  requires a `test(red):` predecessor for anything that introduces a
  `feat` or a `fix`, and spells the verification out as a mechanical
  procedure, but nothing runs it. Unpaired commits have reached
  master; each was caught by a reviewer reading the log by hand, or
  missed.

  Three things trip an implementation, and the first is what let the
  known violations through:

  - **The subject is the type, not the scope.** The rule covers every
    `feat` and `fix`, so a gate keyed on `feat(green):`/`fix(green):`
    misses a bare `fix:` — which is the form every unpaired commit
    took.
  - **Any-red is not the question.** Asking whether the branch
    contains *a* red passes a green whose red landed on a
    separately-merged branch. Ask whether each green has a red
    **ancestor**; `--is-ancestor <green> <red>` tests the negation.
  - **Upstream commits are exempt** (§2). A sync merge imports
    upstream history verbatim, so its `feat` commits have no red of
    ours and cannot acquire one. Provenance is mechanical: reachable
    from the sync merge's second parent.

  Related to the pre-commit and TDD tension above — both are about
  giving the contract teeth instead of restating it.

- **Per-episode audio-mode caps.** Availability answers whether a
  show carries the requested mode, not which of its episodes do, so
  a partly dubbed show advertises its whole listing under the dub
  key and an undubbed episode fails when clicked.

  This is a regression the anidb switch introduced, not longstanding
  behaviour. allanime returned `availableEpisodesDetail` with
  separate `sub` and `dub` arrays, so a single show fetch gave the
  exact per-mode cap and the per-mode extras for free. anidb's
  episode listing carries no audio at all — it lives on each
  episode's languages row — so the same answer now costs one request
  per episode.

  Deriving the prefix anyway was attempted and withdrawn: a
  sub-linear search cannot vouch for rows it never fetched, and
  three review rounds each found the next place where an unfetched
  row got counted (the tail, the front, then anywhere below a
  bisected boundary). Whoever picks this up needs a cheaper source
  of per-episode audio, or a budget for the full scan on listings
  small enough to afford it — not a smarter search over the same
  requests.

## The bundled script's remaining purpose

- **Decide what the ani-cli auto-update setting is for**, now that
  nothing in the app runs the script.

  The toggle is on by default and makes a network request at every
  launch. What it updates is `$XDG_CACHE_HOME/ani-gui/ani-cli` — a
  copy the app maintains for itself. That copy is not on `PATH`, no
  packaging exposes it (the `.deb` postinst symlinks only the
  `ani-gui` launcher), and an `ani-cli` the user installed separately
  is never touched. Playback, downloads and availability all resolve
  natively and read none of it.

  So the setting currently buys a user nothing they can observe. The
  copy describes it accurately, which makes the emptiness visible
  rather than fixing it. Three ways out, and picking one is a
  maintainer call: expose the maintained copy as the terminal
  command so the toggle means what it says; keep the script as an
  inert bundled artifact and retire the updater with its setting and
  its diagnostics panel; or drop the script from the bundle
  altogether and point terminal users at upstream.

## Housekeeping

- **Snapshot `$0`: preserve the basename as well as the directory**, if
  a script ever needs it.
